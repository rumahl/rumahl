FROM busybox@sha256:9db7b59979c38555a39def84a31fb98b5296952f9e3afd4f6f11f05b07adfab0

USER 65534:65534
CMD ["/bin/sh", "-c", "printf ready >/tmp/rumahl-ready; trap 'exit 0' TERM INT; while :; do sleep 1; done"]
