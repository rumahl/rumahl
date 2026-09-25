use std::error::Error;
use std::ffi::{CString, OsString};
use std::fmt;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

use rumahl_platform_buildroot::{
    DockerRuntimeTarget, DockerRuntimeTargetConfig, DockerRuntimeTargetConfigError,
    DockerRuntimeTargetError, NamespaceRuntimeSecretTarget, NamespaceRuntimeSecretTargetConfig,
    NamespaceRuntimeSecretTargetConfigError, StagedDockerImageResolver,
    StagedDockerImageResolverConfig, StagedDockerImageResolverConfigError,
    StagedDockerImageResolverError, UnixRuntimeControlServer, UnixRuntimeControlServerConfig,
    UnixRuntimeControlServerConfigError, UnixRuntimeControlServerError, UnixRuntimeSecretServer,
    UnixRuntimeSecretServerConfig, UnixRuntimeSecretServerConfigError,
    UnixRuntimeSecretServerError,
};

const HELP: &str = "\
Usage: rumahl-runtime-supervisor \\
  --docker PATH \\
  --runtime-root PATH \\
  --image-root PATH \\
  --network NAME \\
  --instance NAME \\
  --control-socket PATH \\
  --secret-socket PATH \\
  --platform-user NAME
";
const MAX_PASSWD_BUFFER: usize = 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
struct SupervisorArgs {
    docker: PathBuf,
    runtime_root: PathBuf,
    image_root: PathBuf,
    network: String,
    instance: String,
    control_socket: PathBuf,
    secret_socket: PathBuf,
    platform_user: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SupervisorArgsError {
    UnknownOption,
    MissingValue,
    DuplicateOption,
    MissingOption(&'static str),
    NonUtf8Value(&'static str),
}

#[derive(Debug)]
enum SupervisorError {
    Arguments(SupervisorArgsError),
    PlatformUserContainsNul,
    PlatformUserLookup(io::Error),
    UnknownPlatformUser,
    ImageResolverConfig(StagedDockerImageResolverConfigError),
    TargetConfig(DockerRuntimeTargetConfigError),
    TargetProbe(DockerRuntimeTargetError<StagedDockerImageResolverError>),
    ServerConfig(UnixRuntimeControlServerConfigError),
    ServerBind(UnixRuntimeControlServerError),
    SecretTargetConfig(NamespaceRuntimeSecretTargetConfigError),
    SecretServerConfig(UnixRuntimeSecretServerConfigError),
    SecretServerBind(UnixRuntimeSecretServerError),
    SecretServerAccept,
}

fn main() -> ExitCode {
    if std::env::args_os().any(|argument| argument == "--help") {
        print!("{HELP}");
        return ExitCode::SUCCESS;
    }
    match run(std::env::args_os().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("rumahl runtime supervisor failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: impl IntoIterator<Item = OsString>) -> Result<(), SupervisorError> {
    let args = parse_args(arguments).map_err(SupervisorError::Arguments)?;
    let platform_uid = user_uid(&args.platform_user)?;
    let resolver = StagedDockerImageResolver::new(
        StagedDockerImageResolverConfig::new(args.image_root)
            .map_err(SupervisorError::ImageResolverConfig)?,
    );
    let runtime_root = args.runtime_root;
    let target = DockerRuntimeTarget::new(
        DockerRuntimeTargetConfig::new(args.docker, &runtime_root, args.network, args.instance)
            .map_err(SupervisorError::TargetConfig)?,
        resolver,
    );
    target.probe().map_err(SupervisorError::TargetProbe)?;
    let server_config = UnixRuntimeControlServerConfig::new(args.control_socket, platform_uid)
        .and_then(|config| config.with_socket_mode(0o660))
        .map_err(SupervisorError::ServerConfig)?;
    let server = UnixRuntimeControlServer::bind(server_config, target)
        .map_err(SupervisorError::ServerBind)?;

    let secret_target = NamespaceRuntimeSecretTarget::new(
        NamespaceRuntimeSecretTargetConfig::new(runtime_root)
            .map_err(SupervisorError::SecretTargetConfig)?,
    );
    let secret_server_config = UnixRuntimeSecretServerConfig::new(args.secret_socket, platform_uid)
        .and_then(|config| config.with_socket_mode(0o660))
        .map_err(SupervisorError::SecretServerConfig)?;
    let secret_server = UnixRuntimeSecretServer::bind(secret_server_config, secret_target)
        .map_err(SupervisorError::SecretServerBind)?;

    std::thread::spawn(move || {
        loop {
            match server.serve_once() {
                Ok(()) => {}
                Err(UnixRuntimeControlServerError::Accept(_)) => {
                    eprintln!("runtime supervisor control listener failed");
                    std::process::exit(1);
                }
                Err(error) => eprintln!("runtime supervisor rejected one control request: {error}"),
            }
        }
    });

    loop {
        match secret_server.serve_once() {
            Ok(()) => {}
            Err(UnixRuntimeSecretServerError::Accept(_)) => {
                return Err(SupervisorError::SecretServerAccept);
            }
            Err(error) => {
                eprintln!("runtime supervisor rejected one secret request: {error}");
            }
        }
    }
}

fn parse_args(
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<SupervisorArgs, SupervisorArgsError> {
    let mut docker = None;
    let mut runtime_root = None;
    let mut image_root = None;
    let mut network = None;
    let mut instance = None;
    let mut control_socket = None;
    let mut secret_socket = None;
    let mut platform_user = None;
    let mut arguments = arguments.into_iter();

    while let Some(option) = arguments.next() {
        let option = option.to_str().ok_or(SupervisorArgsError::UnknownOption)?;
        let value = arguments.next().ok_or(SupervisorArgsError::MissingValue)?;
        match option {
            "--docker" => set_once(&mut docker, PathBuf::from(value))?,
            "--runtime-root" => set_once(&mut runtime_root, PathBuf::from(value))?,
            "--image-root" => set_once(&mut image_root, PathBuf::from(value))?,
            "--network" => set_once(
                &mut network,
                value
                    .into_string()
                    .map_err(|_| SupervisorArgsError::NonUtf8Value("--network"))?,
            )?,
            "--instance" => set_once(
                &mut instance,
                value
                    .into_string()
                    .map_err(|_| SupervisorArgsError::NonUtf8Value("--instance"))?,
            )?,
            "--control-socket" => set_once(&mut control_socket, PathBuf::from(value))?,
            "--secret-socket" => set_once(&mut secret_socket, PathBuf::from(value))?,
            "--platform-user" => set_once(
                &mut platform_user,
                value
                    .into_string()
                    .map_err(|_| SupervisorArgsError::NonUtf8Value("--platform-user"))?,
            )?,
            _ => return Err(SupervisorArgsError::UnknownOption),
        }
    }

    Ok(SupervisorArgs {
        docker: docker.ok_or(SupervisorArgsError::MissingOption("--docker"))?,
        runtime_root: runtime_root.ok_or(SupervisorArgsError::MissingOption("--runtime-root"))?,
        image_root: image_root.ok_or(SupervisorArgsError::MissingOption("--image-root"))?,
        network: network.ok_or(SupervisorArgsError::MissingOption("--network"))?,
        instance: instance.ok_or(SupervisorArgsError::MissingOption("--instance"))?,
        control_socket: control_socket
            .ok_or(SupervisorArgsError::MissingOption("--control-socket"))?,
        secret_socket: secret_socket
            .ok_or(SupervisorArgsError::MissingOption("--secret-socket"))?,
        platform_user: platform_user
            .ok_or(SupervisorArgsError::MissingOption("--platform-user"))?,
    })
}

fn set_once<T>(slot: &mut Option<T>, value: T) -> Result<(), SupervisorArgsError> {
    if slot.replace(value).is_some() {
        return Err(SupervisorArgsError::DuplicateOption);
    }
    Ok(())
}

fn user_uid(name: &str) -> Result<u32, SupervisorError> {
    let name = CString::new(name).map_err(|_| SupervisorError::PlatformUserContainsNul)?;
    // SAFETY: `sysconf` has no pointer preconditions.
    let suggested = unsafe { libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) };
    let mut buffer_length = if suggested > 0 {
        (suggested as usize).clamp(1024, MAX_PASSWD_BUFFER)
    } else {
        16 * 1024
    };

    loop {
        let mut entry = std::mem::MaybeUninit::<libc::passwd>::uninit();
        let mut result = std::ptr::null_mut();
        let mut buffer = vec![0_u8; buffer_length];
        // SAFETY: all pointers reference writable storage for the duration of
        // the call, the name is NUL-terminated, and the buffer length matches.
        let status = unsafe {
            libc::getpwnam_r(
                name.as_ptr(),
                entry.as_mut_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            )
        };
        if status == libc::ERANGE && buffer_length < MAX_PASSWD_BUFFER {
            buffer_length = (buffer_length * 2).min(MAX_PASSWD_BUFFER);
            continue;
        }
        if status != 0 {
            return Err(SupervisorError::PlatformUserLookup(
                io::Error::from_raw_os_error(status),
            ));
        }
        if result.is_null() {
            return Err(SupervisorError::UnknownPlatformUser);
        }
        // SAFETY: a non-null result from successful `getpwnam_r` points to the
        // initialized `entry` supplied above.
        return Ok(unsafe { entry.assume_init() }.pw_uid);
    }
}

impl fmt::Display for SupervisorArgsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownOption => write!(f, "unknown command-line option"),
            Self::MissingValue => write!(f, "command-line option is missing its value"),
            Self::DuplicateOption => write!(f, "command-line option was provided more than once"),
            Self::MissingOption(option) => write!(f, "required option {option} is missing"),
            Self::NonUtf8Value(option) => write!(f, "option {option} must contain UTF-8"),
        }
    }
}

impl fmt::Display for SupervisorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Arguments(error) => write!(f, "invalid configuration: {error}"),
            Self::PlatformUserContainsNul => write!(f, "platform user name is invalid"),
            Self::PlatformUserLookup(_) => write!(f, "platform user lookup failed"),
            Self::UnknownPlatformUser => write!(f, "configured platform user does not exist"),
            Self::ImageResolverConfig(_) => write!(f, "staged image resolver configuration failed"),
            Self::TargetConfig(_) => write!(f, "Docker runtime target configuration failed"),
            Self::TargetProbe(_) => write!(f, "Docker runtime target probe failed"),
            Self::ServerConfig(_) => write!(f, "runtime control server configuration failed"),
            Self::ServerBind(_) => write!(f, "runtime control server startup failed"),
            Self::SecretTargetConfig(_) => {
                write!(f, "runtime secret target configuration failed")
            }
            Self::SecretServerConfig(_) => {
                write!(f, "runtime secret server configuration failed")
            }
            Self::SecretServerBind(_) => write!(f, "runtime secret server startup failed"),
            Self::SecretServerAccept => write!(f, "runtime secret server accept loop failed"),
        }
    }
}

impl Error for SupervisorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Arguments(error) => Some(error),
            Self::PlatformUserLookup(error) => Some(error),
            Self::ImageResolverConfig(error) => Some(error),
            Self::TargetConfig(error) => Some(error),
            Self::TargetProbe(error) => Some(error),
            Self::ServerConfig(error) => Some(error),
            Self::ServerBind(error) => Some(error),
            Self::SecretTargetConfig(error) => Some(error),
            Self::SecretServerConfig(error) => Some(error),
            Self::SecretServerBind(error) => Some(error),
            Self::PlatformUserContainsNul
            | Self::UnknownPlatformUser
            | Self::SecretServerAccept => None,
        }
    }
}

impl Error for SupervisorArgsError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete_args() -> Vec<OsString> {
        [
            "--docker",
            "/usr/bin/docker",
            "--runtime-root",
            "/run/rumahl-runtime-supervisor/apps",
            "--image-root",
            "/var/lib/rumahl-runtime-images",
            "--network",
            "rumahl-apps",
            "--instance",
            "system",
            "--control-socket",
            "/run/rumahl-runtime-supervisor/control.sock",
            "--secret-socket",
            "/run/rumahl-runtime-supervisor/secrets.sock",
            "--platform-user",
            "rumahl-platform",
        ]
        .into_iter()
        .map(OsString::from)
        .collect()
    }

    #[test]
    fn parses_complete_explicit_configuration() {
        let parsed = parse_args(complete_args()).unwrap();
        assert_eq!(parsed.docker, PathBuf::from("/usr/bin/docker"));
        assert_eq!(parsed.network, "rumahl-apps");
        assert_eq!(parsed.platform_user, "rumahl-platform");
    }

    #[test]
    fn rejects_missing_unknown_and_duplicate_options() {
        assert_eq!(
            parse_args(Vec::<OsString>::new()).unwrap_err(),
            SupervisorArgsError::MissingOption("--docker")
        );
        assert_eq!(
            parse_args([OsString::from("--other"), OsString::from("value")]).unwrap_err(),
            SupervisorArgsError::UnknownOption
        );
        let mut duplicate = complete_args();
        duplicate.extend([OsString::from("--network"), OsString::from("other")]);
        assert_eq!(
            parse_args(duplicate).unwrap_err(),
            SupervisorArgsError::DuplicateOption
        );
    }

    #[test]
    fn resolves_existing_and_rejects_unknown_users() {
        // Every supported host has the root account, but the returned UID is
        // read through libc rather than assumed by the supervisor.
        assert_eq!(user_uid("root").unwrap(), 0);
        // Host NSS backends can report an absent user as either no entry or
        // a lookup error. Both must reject the configured user; the exact
        // error classification is not portable across build hosts.
        let lookup = user_uid("rumahl-user-that-must-not-exist-7f3e7c73");
        assert!(
            matches!(
                &lookup,
                Err(SupervisorError::UnknownPlatformUser | SupervisorError::PlatformUserLookup(_))
            ),
            "unknown platform user must be rejected"
        );
    }
}
