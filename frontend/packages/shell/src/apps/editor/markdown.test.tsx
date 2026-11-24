import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test } from "vitest";
import { MarkdownDocument, htmlToMarkdown, isMarkdownFile, markdownToHtml } from "./markdown";

const react = (source: string) => renderToStaticMarkup(<MarkdownDocument source={source} />);
const html = (source: string) => markdownToHtml(source);

function rootOf(source: string): HTMLElement {
  const root = document.createElement("div");
  root.innerHTML = markdownToHtml(source);
  return root;
}

describe("markdown document", () => {
  test("recognises markdown file names", () => {
    expect(isMarkdownFile("notes.md")).toBe(true);
    expect(isMarkdownFile("README.MARKDOWN")).toBe(true);
    expect(isMarkdownFile("page.mdx")).toBe(true);
    expect(isMarkdownFile("notes.txt")).toBe(false);
    expect(isMarkdownFile("README")).toBe(false);
  });

  test("renders headings, emphasis, code and paragraphs", () => {
    const markup = react("# Title\n\nSome **bold**, _italic_, ~~gone~~ and `code`.\n\n## Section");
    expect(markup).toContain("<h1>Title</h1>");
    expect(markup).toContain("<h2>Section</h2>");
    expect(markup).toContain("<strong>bold</strong>");
    expect(markup).toContain("<em>italic</em>");
    expect(markup).toContain("<del>gone</del>");
    expect(markup).toContain("<code>code</code>");
  });

  test("renders lists, tasks, quotes, rules and fenced code", () => {
    const markup = react([
      "- one",
      "- two",
      "",
      "- [x] done",
      "- [ ] open",
      "",
      "> quoted",
      "",
      "---",
      "",
      "```ts",
      "const a = 1;",
      "```"
    ].join("\n"));
    expect(markup).toContain("<ul>");
    expect(markup).toContain("class=\"md-tasks\"");
    expect(markup).toContain("type=\"checkbox\"");
    expect(markup).toContain("checked=\"\"");
    expect(markup).toContain("<blockquote>");
    expect(markup).toContain("<hr/>");
    expect(markup).toContain("language-ts");
    expect(markup).toContain("const a = 1;");
  });

  test("renders tables", () => {
    const markup = react("| Name | Value |\n| --- | --- |\n| A | 1 |");
    expect(markup).toContain("<table>");
    expect(markup).toContain("<th>Name</th>");
    expect(markup).toContain("<td>1</td>");
  });

  test("keeps links and images safe", () => {
    const markup = react("[site](https://example.com) [bad](javascript:alert(1)) ![shot](https://example.com/a.png) ![bad](javascript:alert(1))");
    expect(markup).toContain("href=\"https://example.com\"");
    expect(markup).toContain("rel=\"noreferrer noopener\"");
    expect(markup).toContain("src=\"https://example.com/a.png\"");
    expect(markup).not.toContain("javascript:");
  });

  test("never injects raw HTML", () => {
    const markup = react("<script>alert(1)</script>\n\n<img src=x onerror=alert(1)>");
    expect(markup).not.toContain("<script>");
    expect(markup).not.toContain("<img src=x");
    expect(markup).toContain("&lt;script&gt;");
  });

  test("terminates on unsupported block markers", () => {
    expect(react("|not a table\nnext")).toContain("not a table");
  });

  test("renders hard line breaks inside a paragraph", () => {
    expect(react("first line  \nsecond line")).toContain("<br/>");
  });
});

describe("rich editor projections", () => {
  test("markdownToHtml produces escaped, formatted markup", () => {
    const markup = html("# Title\n\nA **bold** line with `code` and a [link](https://example.com).\n\n- one\n- two");
    expect(markup).toContain("<h1>Title</h1>");
    expect(markup).toContain("<strong>bold</strong>");
    expect(markup).toContain("<code>code</code>");
    expect(markup).toContain("<a href=\"https://example.com\"");
    expect(markup).toContain("<ul><li>one</li><li>two</li></ul>");
    expect(html("<b>raw</b>")).toContain("&lt;b&gt;raw&lt;/b&gt;");
  });

  test("htmlToMarkdown serializes the edited DOM", () => {
    const root = rootOf([
      "# Field notes",
      "",
      "A **short** guide with `code` and a [link](https://example.com).",
      "",
      "- [x] done",
      "- [ ] open",
      "",
      "> quote",
      "",
      "| A | B |",
      "| --- | --- |",
      "| 1 | 2 |"
    ].join("\n"));
    const source = htmlToMarkdown(root);
    expect(source).toContain("# Field notes");
    expect(source).toContain("**short**");
    expect(source).toContain("`code`");
    expect(source).toContain("[link](https://example.com)");
    expect(source).toContain("- [x] done");
    expect(source).toContain("- [ ] open");
    expect(source).toContain("> quote");
    expect(source).toContain("| A | B |");
  });

  test("round-trips a document through the rich surface", () => {
    const source = "# Title\n\nIntro with **bold** and _italic_.\n\n## List\n\n- item\n- [x] task";
    expect(htmlToMarkdown(rootOf(source))).toBe(source);
  });

  test("keeps fenced code and its language", () => {
    const root = rootOf("```ts\nconst a = 1;\n```");
    expect(htmlToMarkdown(root)).toBe("```ts\nconst a = 1;\n```");
  });

  test("drops unsafe links while serializing", () => {
    const root = document.createElement("div");
    root.innerHTML = '<p><a href="javascript:alert(1)">bad</a></p>';
    expect(htmlToMarkdown(root)).toBe("bad");
  });
});
