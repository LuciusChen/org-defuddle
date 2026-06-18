use org_defuddle_core::{parse_html_to_org, DefuddleOptions, IncludeReplies};
use regex::Regex;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug)]
struct FixtureCase {
    fixture: &'static str,
    contains: &'static [&'static str],
    not_contains: &'static [&'static str],
}

#[derive(Debug, Deserialize)]
struct ExpectedMetadata {
    title: String,
    author: String,
    site: String,
    published: String,
}

fn upstream_dir() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("ORG_DEFUDDLE_DEFUDDLE_DIR") {
        return Some(PathBuf::from(path));
    }

    let local_tmp = PathBuf::from("/private/tmp/defuddle-elisp-source");
    local_tmp.is_dir().then_some(local_tmp)
}

#[test]
fn selected_upstream_fixtures_smoke() {
    let Some(defuddle_dir) = upstream_dir() else {
        eprintln!(
            "skipping upstream fixture smoke test; set ORG_DEFUDDLE_DEFUDDLE_DIR to a defuddle checkout"
        );
        return;
    };

    let cases = [
        FixtureCase {
            fixture: "elements--javascript-links",
            contains: &[
                "simple js link",
                "A *bold js link* should keep its inner HTML.",
                "[[https://example.com/page][another page]]",
            ],
            not_contains: &["javascript:"],
        },
        FixtureCase {
            fixture: "codeblocks--code-pre-nesting",
            contains: &["#+begin_src", "#+begin_src typescript", "interface Options"],
            not_contains: &[],
        },
        FixtureCase {
            fixture: "codeblocks--chroma-inline-linenums",
            contains: &[
                "#+begin_src python",
                "p = 61",
                "print(f\"n={p*q}\")",
                "# phi=5760",
            ],
            not_contains: &[" 1p =", " 2q =", "10# phi", "user-select"],
        },
        FixtureCase {
            fixture: "codeblocks--pygments-lineno",
            contains: &[
                "#+begin_src",
                "import torch\nfrom torch import nn",
                "class Model(nn.Module):",
                "super().__init__()",
            ],
            not_contains: &["1import", "10class", "lineno"],
        },
        FixtureCase {
            fixture: "codeblocks--rehype-pretty-copy",
            contains: &[
                "#+begin_src yaml",
                "tags:",
                "review-cycle: 7",
                "complete-date:            # set when marked done",
            ],
            not_contains: &["Copy code", "--copy-icon"],
        },
        FixtureCase {
            fixture: "codeblocks--hljs-header",
            contains: &[
                "#+begin_src sh",
                "#!/bin/bash",
                "read -p \"Enter your name: \" name",
                "echo \"Hello, $name\"",
            ],
            not_contains: &["bash\n#!/bin/bash", "code__copy-button", "code__header"],
        },
        FixtureCase {
            fixture: "code-blocks--chroma-linenums",
            contains: &[
                "#+begin_src cpp",
                "auto process_values",
                "return std::count_if",
                "#+begin_src nasm",
                "process_values():",
                "vpbroadcastb    xmm1",
            ],
            not_contains: &["| 1 |", "| 2 |", "lntable", "rouge-table"],
        },
        FixtureCase {
            fixture: "codeblocks--rouge-linenums",
            contains: &[
                "#+begin_src cpp",
                "// https://godbolt.org/z/9sqM7PvWh",
                "using Int = int;",
                "assert(std::vector<int>{1, 2, 3}.size() == 3);",
            ],
            not_contains: &["| 1", "rouge-gutter", "rouge-table"],
        },
        FixtureCase {
            fixture: "codeblocks--flex-row-gutter",
            contains: &[
                "#+begin_src",
                "AGENTS.md\nARCHITECTURE.md\ndocs/",
                "├── design-docs/",
                "│   └── core-beliefs.md",
                "src/\n└── main.ts",
            ],
            not_contains: &["1AGENTS", "2ARCHITECTURE", "text-primary-44"],
        },
        FixtureCase {
            fixture: "code-blocks--hexo-br",
            contains: &[
                "#+begin_src sh\necho hello\n#+end_src",
                "#+begin_src cpp\n#include <iostream>\n\nint main() {\n  std::cout << \"hello\";\n  return 0;\n}\n#+end_src",
                "#+begin_src\nfoo bar\nbaz qux\n#+end_src",
            ],
            not_contains: &["| 1 |", "1234567", "foo barbaz"],
        },
        FixtureCase {
            fixture: "codeblocks--chroma-line-spans",
            contains: &[
                "#+begin_src go",
                "package main\n\ntype Person struct {",
                "func (p *Person) Sleep() int {",
                "return p.Age",
            ],
            not_contains: &["class=\"line\"", "class=\"cl\""],
        },
        FixtureCase {
            fixture: "codeblocks--rehype-pretty-code",
            contains: &[
                "#+begin_src fish",
                "posts/\n├── 1221499500000000-c5.json",
                "└── 1221499500000001-k3.json   # artificial +1 avoids local collisions",
            ],
            not_contains: &["data-line=\"\"", "style=\"display: grid\""],
        },
        FixtureCase {
            fixture: "codeblocks--react-syntax-highlighter-linenums",
            contains: &[
                "#+begin_src java",
                "// sources/gov/whitehouse/app/BuildConfig.java",
                "public final class BuildConfig {",
                "public static final String VERSION_NAME = \"47.0.1\";",
            ],
            not_contains: &["java\nCopy", "Copy\n\n#+begin_src", "react-syntax-highlighter-line-number"],
        },
        FixtureCase {
            fixture: "codeblocks--chatgpt-codemirror",
            contains: &[
                "#+begin_src python",
                "def fibonacci_generator(n):",
                "        yield a",
                "for num in fibonacci_generator(10):",
            ],
            not_contains: &["Pythondef", "Run code", "code-block-viewer"],
        },
        FixtureCase {
            fixture: "codeblocks--rockthejvm.com-articles-kotlin-101-type-classes",
            contains: &[
                "#+begin_src kotlin",
                "tasks.withType<KotlinCompile>().configureEach {\n    kotlinOptions {",
                "freeCompilerArgs = freeCompilerArgs + \"-Xcontext-receivers\"",
                "data class CreatePortfolioDTO(val userId: String, val amount: Double)",
            ],
            not_contains: &["configureEach {    kotlinOptions", "class=\"ec-line\"", "aria-hidden"],
        },
        FixtureCase {
            fixture: "codeblocks--mintlify",
            contains: &[
                "The ~mode~ prop controls how ~SanityImage~ handles aspect ratio changes",
                "#+begin_src tsx",
                "<SanityImage\n  id={image._id}",
                "mode=\"contain\" // or omit, as this is the default",
                "mode=\"cover\"",
            ],
            not_contains: &["copy-code-button", "Next Steps", "Image Styling"],
        },
        FixtureCase {
            fixture: "codeblocks--stripe",
            contains: &[
                "** Use x402 for machine-to-machine payments.",
                "** Create your endpoint",
                "Add payment middleware to your endpoint",
                "#+begin_src\nimport { paymentMiddleware } from \"@x402/hono\";",
                "config: { description: \"Access to paid content\" }",
                "** Test your endpoint",
                "#+begin_src\ncurl http://localhost:3000/paid\n#+end_src",
                "** Run mainnet transactions",
            ],
            not_contains: &[
                "Ask about this page",
                "Copy for LLM",
                "[[#create-your-endpoint]]",
                "Node.js",
                "Python",
                "No results",
                "Command Line",
                "\n\t\t\t\t\t\t\t\t\t\t\t\t\timport",
            ],
        },
        FixtureCase {
            fixture: "extractor--chatgpt-citations",
            contains: &[
                "** You said",
                "** ChatGPT said",
                "How do I choose a good air purifier for my bedroom?",
                "For a bedroom air purifier, the most important thing",
                "*** The 5 most important things to look for",
                "**** 1. CADR (Clean Air Delivery Rate) — the most important specification",
                "[[https://www.epa.gov/indoor-air-quality-iaq/guide-air-cleaners-home?utm_source=chatgpt.com]",
                "*** Example products worth considering",
                "| Attribute | Coway Airmega 100 ilmanpuhdistin",
            ],
            not_contains: &["Thought for", "Sources", "copy-turn-action-button"],
        },
        FixtureCase {
            fixture: "extractor--chatgpt-post-thought-content",
            contains: &[
                "** You said",
                "** ChatGPT said",
                "Please help me plan a simple weekend picnic.",
                "Start with a simple checklist before choosing the location.",
                "Pick a nearby park, check the weather, and choose food that travels well.",
                "Bring water, napkins, a blanket, and a small bag for cleanup.",
            ],
            not_contains: &["Thought for 12s"],
        },
        FixtureCase {
            fixture: "extractor--bbcode-data",
            contains: &[
                "Patch 1.2.3 is now live! This is build 500",
                "[[https://docs.example.com/patch-123][full patch notes here]]",
                "Or, watch the video patch notes below:",
                "[[https://www.youtube.com/watch?v=dQw4w9WgXcQ]]",
            ],
            not_contains: &["[p]", "[url=", "previewyoutube", "Example Store News"],
        },
        FixtureCase {
            fixture: "comments--news.ycombinator.com-item-id=12345678",
            contains: &[
                "[[https://example.com/article][https://example.com/article]]",
                "** Comments",
                "*commenter_one* · [[https://news.ycombinator.com/item?id=12345679][2025-01-15]] · 25 points",
                "*commenter_two* · [[https://news.ycombinator.com/item?id=12345680][2025-01-15]]",
                "Exactly. And the benchmarks in section 3 back this up nicely.",
                "*commenter_three* · [[https://news.ycombinator.com/item?id=12345682][2025-01-15]] · 10 points",
            ],
            not_contains: &["votearrow", "s.gif", "45 comments"],
        },
        FixtureCase {
            fixture: "general--news.ycombinator.com-item-id=12345678",
            contains: &[
                "*testuser* · 2025-06-15",
                "This is the main comment text that should be extracted",
                "It has multiple paragraphs to test proper content extraction.",
                "And a link: [[https://example.com][https://example.com]]",
            ],
            not_contains: &["parent", "context", "favorite", "Example Story Title"],
        },
        FixtureCase {
            fixture: "listing--news.ycombinator.com-news",
            contains: &[
                "1. [[https://example.com/building-a-database-from-scratch][Building a Database from Scratch in Rust]] (example.com)",
                "384 points · by dev_user · [[https://news.ycombinator.com/item?id=10000001][142 comments]]",
                "3. [[https://news.ycombinator.com/item?id=10000003][Ask HN: What side projects are you working on?]]",
                "[[https://news.ycombinator.com/news?p=2][More]]",
            ],
            not_contains: &["votearrow", "login", "upvote"],
        },
        FixtureCase {
            fixture: "general--daringfireball.net-2025-02-the_iphone_16e",
            contains: &[
                "* The iPhone 16e",
                ":AUTHOR: John Gruber",
                ":PUBLISHED: 2025-02-26T00:00:00+00:00",
                ":SITE: Daring Fireball",
                "In many ways, the iPhone 16e both looks and feels",
                "| iPhone 16e | 7.8mm | — | 9.5mm |",
                "[[https://daringfireball.net/misc/2025/02/iphones-16pro-16reg-16e.png][https://daringfireball.net/misc/2025/02/iphones-16pro-16reg-16e.png]]",
                "** What’s Missing: MagSafe",
                "** Pricing",
                "[fn:1] There’s a decided /feel/ difference",
                "[fn:2] One of the most surprising aspects",
            ],
            not_contains: &[
                "Wednesday, 26 February 2025",
                "Previous:",
                "Next:",
                "Display Preferences",
                "******",
            ],
        },
        FixtureCase {
            fixture: "selectors--arm-newsroom",
            contains: &[
                "* Sample Article",
                ":AUTHOR: Jane Smith",
                ":SITE: Jane Smith",
                "Brief subtitle describing the article.",
                "The company announced a new generation of processors designed specifically for large-scale artificial intelligence workloads",
                "Engineers working on the architecture chose to increase the memory bandwidth available to each core",
                "Software compatibility was a stated priority throughout development.",
            ],
            not_contains: &[
                "Blog",
                "Sample Article Title",
                "Vice President, Example Division",
                "Article Text",
                "Copy Text",
                "Any re-use permitted",
                "Editorial Contact",
                "Latest on X",
                "Sample tweet text",
            ],
        },
        FixtureCase {
            fixture: "elementor--archive-page",
            contains: &[
                "* Apartments for Sale | Updated 2025",
                "PREMIUM APARTMENTS FOR SALE",
                "** Premium Apartment Market Overview Q1/2026",
                "[[https://example.com/market-report/][The premium apartment market report]]",
                "| *Indicator* | *Q1/2026 Updated Data* |",
                "** Premium Apartment Projects Currently For Sale",
                "**** Riverside Heights",
                "** Important Notes When Buying Premium Apartments",
                "**** Should you choose riverside or beachfront apartments?",
            ],
            not_contains: &[
                "Current Premium Apartment Projects",
                "Template Item",
                "Leaflet",
                "Site Logo",
                "The City is Waiting for You",
                "Contact us for expert consultation",
            ],
        },
        FixtureCase {
            fixture: "general--scp-wiki.wikidot.com-scp-9935",
            contains: &[
                "* SCP-9935 - SCP Foundation",
                "[[https://scp-wiki.wdfiles.com/local--files/scp-9935/baseballbanner.jpg][baseballbanner.jpg]]",
                "Bruce Park, Indianapolis, 1889.",
                "| *Assigned Department* | *Department Head* | *Research Head* | *Assigned Task Force* |",
                "*Description:* SCP-9935 is an anomalous baseball game between the Indianapolis Hoosiers and the Washington Nationals [fn:1]",
                "** Rosters",
                "[fn:1] No relation to the Washington Nationals team of the modern day.",
                "[fn:3] Jack Glasscock, Hoosiers shortstop and team captain.",
            ],
            not_contains: &[
                "\nSCP-9935\n",
                "javascript:",
                "footnoteref",
                "Footnotes\n",
            ],
        },
        FixtureCase {
            fixture: "general--github.com-issue-56",
            contains: &[
                "* Defuddle on Cloudflare Workers · Issue #56",
                ":AUTHOR: jmorrell",
                ":SITE: GitHub",
                "[[https://github.com/jmorrell/defuddle-cloudflare-example][https://github.com/jmorrell/defuddle-cloudflare-example]]",
                "~readbilityjs~ fork",
                "Since defuddle relies on these style heuristics, *I'm not sure there is a great path to supporting the full functionality in this environment*",
                "#+begin_src",
                "Defuddle Error processing document: TypeError: e3.getComputedStyle is not a function",
            ],
            not_contains: &[
                "Sign up for free",
                "Issue body actions",
                "Metadata",
                "copy",
                "New issue",
            ],
        },
        FixtureCase {
            fixture: "general--github.com-test-owner-test-repo-pull-42",
            contains: &[
                "* Fix rendering when malformed elements nest content · Pull Request #42 · test-owner/test-repo",
                ":AUTHOR: author-one",
                ":SITE: GitHub - test-owner/test-repo",
                ":PUBLISHED: 2026-01-15T10:30:00Z",
                "** Summary",
                "The root cause was a malformed ~<figure>~ in the source HTML.",
                "- Preserve remaining content after extraction",
                "** Comments",
                "*reviewer-bot* · 2026-01-15T10:45:00Z",
                "Consider removing just the image element instead of the entire anchor",
                "*author-one* · 2026-01-15T14:00:00Z",
                "- Preserve linked text when stripping the image",
            ],
            not_contains: &[
                "timeline-comment-header",
                "js-comment-body",
                "commented Jan 15",
            ],
        },
        FixtureCase {
            fixture: "general--substack-app",
            contains: &[
                "* Rich Holmes (@richholmes)",
                ":AUTHOR: Substack",
                ":SITE: Substack",
                "Google's former CEO says traditional user interfaces",
                "*Why this matters for product teams*",
                "Designers focus less on screen layouts, more on flexible component libraries.",
                "*The strategic implications*",
                "[[https://departmentofproduct.substack.com/p/agent-driven-user-interfaces-explained][departmentofproduct.sub…]]",
            ],
            not_contains: &[
                "The app for independent voices",
                "Get started",
                "You made it, you own it",
                "160 Likes",
            ],
        },
        FixtureCase {
            fixture: "general--substack-custom-domain",
            contains: &[
                "* Test Article",
                ":AUTHOR: Test Author",
                ":SITE: Example Newsletter",
                ":PUBLISHED: 2025-06-15T10:00:00+00:00",
                "** First Section",
                "Nemo enim ipsam voluptatem quia voluptas sit aspernatur",
                "-----",
                "** Second Section",
                "Ut enim ad minima veniam, quis nostrum exercitationem ullam corporis",
            ],
            not_contains: &[
                "Share",
                "For More on This Topic",
                "Related Article Title",
                "Read full story",
                "Subscribe",
                "Type your email",
            ],
        },
        FixtureCase {
            fixture: "general--12gramsofcarbon.com-p-ilyas-30-papers-to-carmack-vlaes",
            contains: &[
                "* Ilya's Papers to Carmack: VLAEs",
                ":AUTHOR: theahura",
                ":SITE: 12 Grams of Carbon",
                ":PUBLISHED: 2025-03-06T12:48:40+00:00",
                "This post is part of a series of paper reviews",
                "* Paper 18: Variational Lossy Autoencoder",
                "A well structured representation space",
                "Because latent variables learn a 'space' of representations",
                "[fn:1] Note that there is a bit of a blurred line",
                "[fn:6] Spoiler: the coffee automaton paper",
                "ffb5afd7-ffc8-402d-8eaa-4060938fe1d2",
            ],
            not_contains: &[
                "Subscribe for free",
                "Discussion about this post",
                "Ready for more",
                "footnote-anchor",
                "[[https://12gramsofcarbon.com/p/ilyas-30-papers-to-carmack-vlaes#footnote-anchor",
            ],
        },
        FixtureCase {
            fixture: "general--substack-note",
            contains: &[
                "* Test User (@testuser)",
                ":AUTHOR: Test User",
                ":SITE: Substack",
                "Sample note text for testing the Substack extractor.",
                "It has multiple paragraphs to verify the content is captured correctly.",
                "[[https://example.com/image/full-res.jpg]]",
            ],
            not_contains: &["avatar.jpg", "42", "Like", "Restack", "Share"],
        },
        FixtureCase {
            fixture: "general--substack-note-permalink",
            contains: &[
                "* Test User (@testuser)",
                ":AUTHOR: Test User",
                ":SITE: Substack",
                "This is the main note content on the permalink page.",
                "It has multiple paragraphs with important information.",
                "[[https://example.com/image/main-note.jpg]]",
            ],
            not_contains: &[
                "This is a different note from the feed sidebar.",
                "Another unrelated feed note",
                "Yet another unrelated feed note",
                "avatar.jpg",
                "Like",
                "Restack",
            ],
        },
        FixtureCase {
            fixture: "issues--141-arxiv-equation-tables",
            contains: &[
                "* arXiv Equation Tables",
                "** Scaled Dot-Product Attention",
                "$$\n\\mathrm{Attention}(Q,K,V)=\\mathrm{softmax}(\\frac{QK^{T}}{\\sqrt{d_{k}}})V\n$$",
                "The two most commonly used attention functions are additive attention",
                "$$\n\\mathrm{MultiHead}(Q,K,V)=\\mathrm{Concat}(\\mathrm{head}_{1},...,\\mathrm{head}_{h})W^{O}\n$$",
            ],
            not_contains: &[
                "|  | $$",
                "|  | $\\mathrm{MultiHead}",
                "ltx_eqn_table",
            ],
        },
        FixtureCase {
            fixture: "issues--142-arxiv-multi-citations",
            contains: &[
                "* arXiv Multi-Citations",
                "** Introduction",
                "prior work [35, 2, 5]",
                "others reference two works [7, 9]",
                "** References",
                "Dzmitry Bahdanau, Kyunghyun Cho, and Yoshua Bengio",
                "Ilya Sutskever, Oriol Vinyals, and Quoc V. Le",
            ],
            not_contains: &[
                "[[[https://arxiv.org",
                "[[#bib.bib",
                "ltx_ref",
            ],
        },
        FixtureCase {
            fixture: "issues--143-arxiv-cross-references",
            contains: &[
                "* arXiv Cross-References",
                "** Model Architecture",
                "Figure 1, respectively",
                "Section 3.2",
                "Table 1.",
            ],
            not_contains: &["[[#S3.F1]]", "[[#S3.SS2]]", "[[#S3.T1]]"],
        },
        FixtureCase {
            fixture: "issues--144-arxiv-footnote-marks",
            contains: &[
                "* arXiv Footnote Marks",
                "** Authors",
                "Rafael Rafailov, Archit Sharma, Eric Mitchell",
                "This is the actual article content that follows the author section.",
            ],
            not_contains: &[
                "footnotemark",
                "ltx_note",
                "Rafael Rafailov 2",
            ],
        },
        FixtureCase {
            fixture: "author-contact-block",
            contains: &[
                "Researchers have developed a new technique for converting microwave photons to optical photons",
                "The device bridges the significant energy gap between the microwave domain",
                "Graduate student Maria Lopez is also an author of the paper describing the work",
            ],
            not_contains: &[
                "\nWritten by\n",
                "\nContact\n",
                "(555) 123-4567",
                "jsmith@example.edu",
            ],
        },
        FixtureCase {
            fixture: "author-share-widget",
            contains: &[
                "This is the first paragraph of a long blog post",
                "** Background",
                "** New Features",
                "We believe these changes will make a real difference for our users",
            ],
            not_contains: &[
                "\nAuthor\n",
                "\nShare\n",
                "Jane Smith Headshot",
                "jane-smith.jpg",
            ],
        },
        FixtureCase {
            fixture: "gated-content--cookie-consent",
            contains: &[
                "The company on Thursday began removing advertisements",
                "*Why it matters:* This comes just two weeks after the company was [[https://example.com/verdict][found negligent]]",
                "*Driving the news:* Reporters have identified more than a dozen such ads",
                "*The bottom line:* While the company has every right",
            ],
            not_contains: &[
                "Manage your tracker preferences",
                "Targeted advertising cookies",
                "What to read next",
                "Newsletters",
            ],
        },
        FixtureCase {
            fixture: "content-patterns--code-block-boilerplate-and-trailing-section",
            contains: &[
                "* Lessons from Building API Integrations",
                "** Code Example: Retry Logic with Logging",
                "#+begin_src python",
                "# comments explaining retry logic",
                "raise MaxRetriesExceeded",
                "We hope these lessons save you time on your own integration projects.",
            ],
            not_contains: &[],
        },
        FixtureCase {
            fixture: "general--stephango.com-buy-wisely",
            contains: &["Cost per use", "[[https://darntough.com/][Darn Tough]]"],
            not_contains: &["5 minute read"],
        },
        FixtureCase {
            fixture: "comments--mastodon.social-@user-12345678",
            contains: &[
                "* Post by Alice on mastodon.example",
                ":AUTHOR: Alice",
                ":SITE: mastodon.example",
                ":PUBLISHED: 2026-04-20",
                "This is a sample post about something interesting.",
                "[[https://example.com/interesting][example.com/interesting]]",
                "[[https://cdn.mastodon.example/media/original/sample-image.png][A sample screenshot showing the project]]",
                "Here is some more context about the project",
                "** Comments",
                "*Bob @bob@other.social* · [[https://mastodon.example/@bob@other.social/12345680][2026-04-20]]",
                "[[https://mastodon.example/@carol@third.social][@carol]] Good question!",
                "*Dave @dave@mastodon.example* · [[https://mastodon.example/@dave/12345683][2026-04-20]]",
                "[[https://example.com/related][https://cdn.mastodon.example/cards/related-preview.png]]",
                "[[https://example.com/related][Related Project]]",
                "A similar project with different goals",
            ],
            not_contains: &[
                "detailed-status__reblogs",
                "detailed-status__favorites",
                "status__action-bar",
                "avatars/alice.png",
                "media/small/sample-image.png",
            ],
        },
        FixtureCase {
            fixture: "comments--old.reddit.com-r-test-comments-abc123-test_post",
            contains: &[
                "* Test Post",
                ":AUTHOR: poster_user",
                ":SITE: r/test",
                ":PUBLISHED: 2025-01-15T10:30:00Z",
                "This is the post body with some content.",
                "** Comments",
                "*user_alpha* · [[https://reddit.com/r/test/comments/abc123/test_post/comment1/][2025-01-15]] · 42 points",
                "*user_beta* · [[https://reddit.com/r/test/comments/abc123/test_post/comment2/][2025-01-15]] · 15 points",
                "*user_gamma* · [[https://reddit.com/r/test/comments/abc123/test_post/comment4/][2025-01-15]] · 3 points",
                "*user_delta* · [[https://reddit.com/r/test/comments/abc123/test_post/comment5/][2025-01-15]] · 20 points",
                "Another top-level comment with a [[https://example.com/][link]].",
            ],
            not_contains: &["Test Post : test", "[[/user/user_alpha][user_alpha]]"],
        },
        FixtureCase {
            fixture: "general--x.com-article",
            contains: &[
                "* Lorem Ipsum Dolor Sit Amet",
                ":AUTHOR: Jane Doe (@janedoe)",
                ":SITE: X (Twitter)",
                "Lorem ipsum dolor sit amet, consectetur adipiscing elit.",
                "*Ut enim ad minim:* Veniam quis nostrud exercitation ullamco",
                "[[https://example.com/media/placeholder.jpg?format=jpg&name=large][Placeholder image]]",
                "Duis aute irure dolor in reprehenderit",
            ],
            not_contains: &["name=medium", "Test X Article"],
        },
        FixtureCase {
            fixture: "general--x.com-article-2026-02-13",
            contains: &[
                "* obsidian + claude code 101",
                ":AUTHOR: Heinrich",
                ":SITE: X (Twitter)",
                ":PUBLISHED: 2026-01-19T00:28:01.000Z",
                "[[https://pbs.twimg.com/media/G-_GGKzXkAAHz_w?format=jpg&name=large][Image]]",
                "ive spent the last year building an operating system for thinking with ai",
                "** knowledge = code?",
                "#+begin_src markdown\nmy-vault/",
                "[[https://claude.md/][CLAUDE.md]] file that teaches the agent",
                "** how the agent operates",
                "** tldr",
            ],
            not_contains: &[
                "X (formerly Twitter)",
                "#+begin_src markdown\nmarkdown\nmy-vault/",
                "\n76\n234\n2.3K\n1.1M\n",
                "Relevant people",
                "Trending now",
                "Copy to clipboard",
                "Follow back",
            ],
        },
        FixtureCase {
            fixture: "issues--161-x-status-url-author",
            contains: &[
                "* Lorem Ipsum Dolor Sit Amet",
                ":URL: https://x.com/janedoe/status/1234567890123456789",
                ":AUTHOR: @janedoe",
                ":SITE: X (Twitter)",
                "Lorem ipsum dolor sit amet, consectetur adipiscing elit.",
            ],
            not_contains: &["Jane DoeさんはXを使っています"],
        },
        FixtureCase {
            fixture: "issues--272-x-localized-conversation-timeline",
            contains: &[
                "* Post by @main_user on X",
                ":URL: https://x.com/main_user/status/1234567890",
                ":AUTHOR: @main_user",
                ":PUBLISHED: 2026-05-16T12:00:00.000Z",
                "Main post from a localized X interface.",
                "** Comments",
                "*Reply User @reply_user* · [[https://x.com/reply_user/status/1234567891][2026-05-16]]",
                "Reply that should be extracted from the localized timeline.",
            ],
            not_contains: &["时间线：对话", "[[/reply_user][Reply User]]"],
        },
        FixtureCase {
            fixture: "general--cp4space-jordan-algebra",
            contains: &[
                "* The exceptional Jordan algebra",
                ":AUTHOR: apgoucher",
                ":SITE: Complex Projective 4-Space",
                ":PUBLISHED: 2020-10-28T14:19:38+00:00",
                "$\\lambda, \\mu \\in \\mathbb{R}$",
                "$A, B$",
                "$AB$",
                "$A \\circ B = \\frac{1}{2}(AB + BA)$",
                "*** Projective spaces from Jordan algebras",
                "This exceptional Jordan algebra is $\\mathfrak{h}_3(\\mathbb{O})$",
                "form an isometric copy of the Leech lattice",
                "*** Further reading",
                "$\\eta = \\dfrac{-1 + \\sqrt{-7}}{2}$",
            ],
            not_contains: &[
                "Posted on",
                "This entry was posted in",
                "latex.php",
                "A blog. Mostly maths",
                "Leave a Reply",
            ],
        },
        FixtureCase {
            fixture: "general--www.figma.com-blog-introducing-codex-to-figma",
            contains: &[
                "* Building Frontend UIs with Codex and Figma",
                ":SITE: Figma",
                ":PUBLISHED: February 26, 2026",
                "c93d4cf38afc223b52c3f1e2a63810e666971bc0",
                "With Codex to Figma, teams can bring real, running interfaces",
                "[[https://www.figma.com/blog/introducing-figma-mcp-server/][Figma MCP server]]",
                "** Starting an app from a design",
                "~Help me implement this Figma design in code",
                "** From code to canvas",
                "*Entire screen:* Capture the render",
                "** There and back again, an MCP story",
                "[[https://developers.figma.com/docs/figma-mcp-server/tools-and-prompts/#generate_figma_design][developer docs]]",
            ],
            not_contains: &[
                "| Figma Blog",
                "Subscribe to Figma’s editorial newsletter",
                "Related articles",
                "Create and collaborate with Figma",
                "Yarden is a Product Manager",
                "LinkedIn",
                "Featured Topics",
                "Categories",
                "data:image/png;base64",
                "Get started for free",
            ],
        },
        FixtureCase {
            fixture: "general--lesswrong.com-s-N7nDePaNabJdnbXeE-p-vJFdjigzmcXMhNTsx",
            contains: &[
                "* Simulators — LessWrong",
                ":AUTHOR: janus",
                ":SITE: janus",
                ":PUBLISHED: 2022-09-02T12:45:33.723Z",
                "/Thanks to Chris Scammell",
                "/This work was carried out while at/",
                "ytdyqqynryhcq1ysqbtk.png",
                "Moebius illustration of a simulacrum living in an AI-generated story",
                "** Summary",
                "*TL;DR*: Self-supervised learning may create AGI or its foundation.",
                "** Meta",
                "The limit of sequence modeling",
                "Claude Shannon described using [[https://en.wikipedia.org/wiki/N-gram][n-grams]]",
                "[fn:1] [[https://www.princeton.edu/~wbialek/rome/refs/shannon_51.pdf][Prediction and Entropy of Printed English]]",
            ],
            not_contains: &[
                "DelayedLoading-spinner",
                "PostBottomRecommendations",
                "PingbacksList",
                "Load More",
            ],
        },
        FixtureCase {
            fixture: "hidden--nodes",
            contains: &["Lorem ipsum dolor sit amet"],
            not_contains: &["Secondary header", "Third header", "hidden=\"hidden\""],
        },
        FixtureCase {
            fixture: "hidden--visibility",
            contains: &[
                "* Lorem",
                "** Foo",
                "Tempor incididunt ut labore et dolore magna aliqua.",
                "Duis aute irure dolor in reprehenderit in voluptate velit esse",
            ],
            not_contains: &[
                "Lorem ipsum dolor sit amet, consectetur adipisicing elit",
                "Iframe fallback test",
                "foo.swf",
            ],
        },
        FixtureCase {
            fixture: "issues--162-aria-hidden-main-content",
            contains: &[
                "Hello, we believe in building in the open",
                "Over the coming months, we will publish architecture notes",
            ],
            not_contains: &["FPS: --", "EARENDIL INC."],
        },
        FixtureCase {
            fixture: "issues--232-dismiss-in-hidden-content",
            contains: &[
                "From: Example <hello@example.com>",
                "We are excited to share an update on our work.",
                "Our goal is to create software that earns trust over time.",
                "[[https://example.com/][example.com]]",
            ],
            not_contains: &[
                "logo.svg",
                "ANNOUNCING THINGS",
                "Dismiss",
                "EXAMPLE INC.",
            ],
        },
        FixtureCase {
            fixture: "issues--header-with-subtitle-p",
            contains: &[
                "When the original first-person shooter was released in 1992",
                "** The control problem",
                "For a one-handed player, this presents an immediate challenge.",
                "** Accessibility in retrospect",
            ],
            not_contains: &[
                "Gaming",
                "Over three decades later, this historical curiosity",
                "Alex Johnson –",
                "Copyright 2026 Example Tech",
            ],
        },
        FixtureCase {
            fixture: "issues--106-menu-id",
            contains: &[
                "Welcome to our restaurant",
                "** Appetizers",
                "Spring rolls with sweet chili sauce",
                "** Main Course",
                "Grilled salmon with roasted vegetables",
                "** Desserts",
                "Chocolate lava cake with vanilla ice cream",
            ],
            not_contains: &["Copyright 2025"],
        },
        FixtureCase {
            fixture: "issues--131-category-links",
            contains: &[
                "This is the main content of the blog post about web development.",
                "[[https://131-category-links/category/web-development][Web Development]]",
                "[[https://131-category-links/categories/javascript][JavaScript]]",
                "[[https://131-category-links/category/tutorials][Tutorials]]",
            ],
            not_contains: &[],
        },
        FixtureCase {
            fixture: "issues--132-hero-class",
            contains: &[
                "This is the hero section with important introductory content that should not be removed.",
                "This is the main body of the article.",
            ],
            not_contains: &[],
        },
        FixtureCase {
            fixture: "issues--136-time-element",
            contains: &[
                "This article explores how time elements should be handled",
                "the event happened at 10:00 AM and ended at 2:00 PM",
                "The 3 months ago update was significant",
                "Content extraction tools must carefully distinguish",
            ],
            not_contains: &["January 15, 2025", "15 Jan, 2025"],
        },
        FixtureCase {
            fixture: "standardize--span-data-as-paragraph",
            contains: &[
                "Cloud sessions start from a fresh clone of your repository.",
                "not.\n\nSessions run on managed cloud infrastructure.",
                "[[https://example.com/docs/get-started][getting started guide]]",
                "** What is available",
                "session.\n\nYou can extend the environment with a setup script.",
            ],
            not_contains: &[
                "not. Sessions run on managed cloud infrastructure.",
                "session. You can extend the environment",
            ],
        },
        FixtureCase {
            fixture: "issues--span-with-block-children-and-schema",
            contains: &[
                "[[https://example.org/images/sculpture.jpg][https://example.org/images/sculpture.jpg]] Systems come in many forms.",
                "The key insight is that constraints are not inherently limiting.",
                "*** Rigid",
                "[[https://example.org/images/rigid.png][https://example.org/images/rigid.png]] Rigid systems appear stable",
                "*** Elastic",
                "The most resilient systems combine multiple constraint types",
            ],
            not_contains: &[
                "[[https://example.org/images/hero.jpg]]",
                "*** Recent Posts",
                "Other Post Title",
                "A brief summary of another post",
                "Read More",
                "About the Company",
            ],
        },
        FixtureCase {
            fixture: "elements--lazy-image",
            contains: &[
                "CPU profiling is a must-have",
                "[[http://voodoo.io/][Voodoo]]",
            ],
            not_contains: &["/?source=post_page"],
        },
        FixtureCase {
            fixture: "elements--srcset-normalization",
            contains: &[
                "React SSR camelCase srcSet attributes",
                "[[https://www.example.com/images/hero.webp][Hero image with React SSR attributes.]]",
                "Hero image with React SSR attributes. Photo credit.",
            ],
            not_contains: &["data:image", "hero-small.webp"],
        },
        FixtureCase {
            fixture: "elements--base64-placeholder-removal",
            contains: &[
                "Base64 Placeholder Images",
                "[[https://www.example.com/images/resolved.webp][Resolvable from picture source.]]",
                "[[https://www.example.com/images/lazy-loaded.jpg][Resolvable from data-src.]]",
                "[[https://www.example.com/images/real-image.jpg][A real image.]]",
            ],
            not_contains: &["Unresolvable placeholder", "data:image"],
        },
        FixtureCase {
            fixture: "elements--svg-placeholder-lazy-image",
            contains: &[
                "Test App startup page",
                "[[https://i.example.com/imagery/reviews/abc123-17.png][File browser in Test App]]",
                "[[https://i.example.com/imagery/reviews/abc123-18.png][Graph view in Test App]]",
                "[[https://i.example.com/imagery/reviews/abc123-20.png][A plugin in Test App]]",
                "[[https://i.example.com/imagery/reviews/abc123-17.png][File browser in Test App]]\n\nFile browser in Test App (Credit: ExampleMag)",
            ],
            not_contains: &["data:image/svg+xml"],
        },
        FixtureCase {
            fixture: "issues--221-nextjs-noscript-images",
            contains: &[
                "Here is an architecture diagram:",
                "[[https://www.example.com/images/architecture.png?imwidth=3840][Architecture diagram.]]",
                "[[https://www.example.com/images/flow.png?imwidth=3840][Flow diagram.]]",
            ],
            not_contains: &["data:image"],
        },
        FixtureCase {
            fixture: "issues--227-noscript-lazy-images",
            contains: &[
                "[[https://www.example.com/images/hero.webp][Hero image caption.]]",
                "[[https://www.example.com/images/gallery-1.jpg][First gallery image caption.]]",
                "[[https://www.example.com/images/gallery-2.jpg][Second gallery image caption.]]",
                "[[https://www.example.com/images/inline-photo.jpg][Inline photo caption.]]",
            ],
            not_contains: &["data:image"],
        },
        FixtureCase {
            fixture: "elements--image-dedup",
            contains: &[
                "[[https://www.example.com/images/photo-large.webp][A landscape photo.]]",
                "[[https://www.example.com/images/portrait-large.webp]]",
                "[[https://www.example.com/latex/formula.png][E equals mc squared]]",
            ],
            not_contains: &["photo-small.jpg", "portrait-small.jpg"],
        },
        FixtureCase {
            fixture: "elements--lightbox-image-dedup",
            contains: &[
                "Gallery Post",
                "[[https://www.example.com/uploads/photo-one.jpg][https://www.example.com/uploads/photo-one-1280x720.jpg]]",
                "[[https://www.example.com/uploads/photo-two.jpg][https://www.example.com/uploads/photo-two-1280x720.jpg]]",
                "[[https://www.example.com/uploads/inline-photo.jpg][An inline photo]]",
            ],
            not_contains: &[
                "[[https://www.example.com/uploads/photo-one.jpg]]",
                "[[https://www.example.com/uploads/photo-two.jpg]]",
                "xmark",
            ],
        },
        FixtureCase {
            fixture: "elements--embedded-videos",
            contains: &[
                "A YouTube video:",
                "[[https://www.youtube.com/watch?v=b_PXuEPxN50]]",
                "A tweet:",
                "[[https://x.com/i/status/1675626836821409792]]",
                "[[https://x.com/kepano/status/1675626836821409792]]",
                "#+begin_export html\n<iframe src=\"https://player.vimeo.com/video/45725193?h=a290f71a57\"",
            ],
            not_contains: &["youtube.com/embed", "youtube-nocookie.com/embed", "platform.twitter.com/embed"],
        },
        FixtureCase {
            fixture: "issues--254-empty-video",
            contains: &[
                "This fixture models a page that includes a JavaScript-driven video shell",
                "The player above should be removed because it has no src attribute",
            ],
            not_contains: &["<video", "player.start"],
        },
        FixtureCase {
            fixture: "issues--286-generic-alt-image-dedup",
            contains: &[
                "This walkthrough shows each step of the workflow with a screenshot.",
                "[[https://image.example.com/2026/04/step_1.png][image]]",
                "[[https://image.example.com/2026/04/step_2.png][image]]",
                "[[https://image.example.com/2026/04/step_3.png][image]]",
                "[[https://image.example.com/2026/04/step_4.png][image]]",
                "[[https://image.example.com/2026/04/step_5.png][image]]",
            ],
            not_contains: &["step_1.png][image]]\n\n[[https://image.example.com/2026/04/step_1.png"],
        },
        FixtureCase {
            fixture: "custom-elements--swiper-carousel",
            contains: &[
                "Some introductory text about the topic.",
                "Gallery",
                "[[https://example.com/a.jpg]]",
                "[[https://example.com/b.png]]",
                "[[https://example.com/c.jpg]]",
                "Text after the carousel.",
            ],
            not_contains: &["swiper-container", "swiper-slide"],
        },
        FixtureCase {
            fixture: "issues--114-leading-hr",
            contains: &[
                "The web browser is the most used program on any desktop.",
                "For such users, a browser extension for translation is a must-have tool.",
                "** Features",
                "Linguist supports full page translation",
            ],
            not_contains: &["-----"],
        },
        FixtureCase {
            fixture: "table-layout--single-column",
            contains: &[
                "* Welcome",
                "Hello, welcome to my website!",
                "** Projects",
                "- [[https://example.com/project-one][Project One]] - a cool project",
                "- [[https://example.com/project-two][Project Two]] - another project",
                "** Contact",
                "Find me on [[https://example.com/social][social media]]",
            ],
            not_contains: &["| Welcome Hello", "| Projects", "| Contact"],
        },
        FixtureCase {
            fixture: "table-layout--paulgraham.com-makersschedule",
            contains: &[
                "\"...the mere consciousness of an engagement will sometimes worry a whole day.\"",
                "July 2009",
                "One reason programmers dislike meetings so much is that they're on a different type of schedule",
                "[[http://ycombinator.com][Y Combinator]] runs on the maker's schedule",
                "*Thanks* to Sam Altman",
                "*Related:*",
            ],
            not_contains: &[
                "| [[https://s.turbifycdn.com/aah/paulgraham/bel-7.gif]]",
                "maker-s-schedule-manager-s-schedule-3.gif",
                "Turkish Translation",
                "French Translation",
            ],
        },
        FixtureCase {
            fixture: "table-layout--blogger-two-column",
            contains: &[
                "* Thoughts on various topics",
                "*Note:* Comments are now moderated.",
                "*** Thursday, May 27, 2004",
                "#+begin_quote",
                "*The One Minute Guide to Avoiding Bad Projects*",
                "1 We also learned that the distinction between making a single false claim",
            ],
            not_contains: &[
                "*Links:*",
                "Friend's Blog",
                "If you liked this website",
                "August 2002",
            ],
        },
        FixtureCase {
            fixture: "table-layout--peripheral-tables",
            contains: &[
                "*** Dr. A. Researcher, March 2015",
                "This article provides an overview of cellular biology with a focus on practical applications",
                "** Cell structure",
                "*** Cell membrane",
                "[[https://example.com/cell-cycle.png][Diagram of cell cycle phases]]",
                "** References",
                "- Alberts, B. et al. (2014). /Molecular Biology of the Cell/.",
            ],
            not_contains: &[
                "| This article provides an overview of cellular biology",
                "** Contents",
                "[[#Cell_structure]",
            ],
        },
        FixtureCase {
            fixture: "elements--data-table",
            contains: &[
                "| Language | Year | Typing | Primary Use |",
                "| Python | 1991 | Dynamic | General purpose, data science |",
                "| Rust | 2015 | Static | Systems programming |",
                "| Name | Score |",
                "| Alice | 95 |",
            ],
            not_contains: &["Programming Language Comparison Here is a comparison"],
        },
        FixtureCase {
            fixture: "elements--complex-tables",
            contains: &[
                "| [[https://example.com/alpha][Alpha]] | 12 | 2019 | Native | *Fastest overall* |",
                "| [[https://example.com/bravo][Bravo]] | 25 | 2020 | JIT | Uses *aggressive caching* |",
                "| Delta *optimized* | 55 | 2021 | Native | Requires [[https://example.com/config][custom config]] |",
                "| [[https://example.com/echo][Echo]] | 90 | 2017 | VM | *Most popular* in surveys |",
                "** Memory Usage",
                "| Framework | Peak Memory (MB) | Idle Memory (MB) |",
            ],
            not_contains: &[
                "| Alpha | 12 | 2019 | Native | Fastest overall |",
                "| Delta optimized |",
                "Requires custom config |",
            ],
        },
        FixtureCase {
            fixture: "scoring--table-with-links",
            contains: &[
                "* Model Comparison",
                "| Size | Patch Size | Resolution | Framework A | Framework B |",
                "| Base (86M) | 32 | 256 | [[https://example.com/model-base-32-256-a][org/model-base-patch32-256]] |",
                "| SO400M (400M) | 14 | 768 | [[https://example.com/model-so400m-14-768-a][org/model-so400m-patch14-768]] |",
                "Each model variant is optimized for different use cases",
            ],
            not_contains: &["About Contact"],
        },
        FixtureCase {
            fixture: "scoring--related-posts-byline",
            contains: &[
                "** How Acme Corp improved performance with a new architecture",
                "By integrating a new microservices architecture",
                "** Key highlights",
                "forty percent",
            ],
            not_contains: &[
                "How a database team reduced query latency",
                "Related posts",
            ],
        },
        FixtureCase {
            fixture: "issues--300-nested-layout-tables",
            contains: &[
                "* Overview of the editor's shortcuts",
                "*** The keystrokes and their functions",
                "*File handling*",
                "| Ctrl+S | Save current file |",
                "| Ctrl+X | Close buffer, exit from the editor |",
                "*Moving around*",
                "| Ctrl+N | One line down |",
                "*Information*",
                "| Alt+D | Report line/word/character count |",
            ],
            not_contains: &[
                "*File handling* \\vert{} Ctrl+S",
                "| *File handling*",
                "\\vert{} *Editing*",
            ],
        },
        FixtureCase {
            fixture: "issues--284-table-cell-header-scoring",
            contains: &[
                "* example::size_t",
                "| Defined in header ~<cstddef>~ |  |  |",
                "| Defined in header ~<cstdlib>~ |  |  |",
                "| ~typedef /* implementation-defined */ size_t;~ |  | (since C++17) |",
                "~example::size_t~ is the unsigned integer type",
                "*** Notes",
            ],
            not_contains: &[
                "#+begin_src typedef",
                "| #+begin_src",
                "Defined in header ~<cstddef>~ | Defined in header",
            ],
        },
        FixtureCase {
            fixture: "issues--217-writerside-docs",
            contains: &[
                "** Methods",
                "Use these methods to interact with the API.",
                "[[https://developer.example.com/docs/getItems][getItems()]]",
                "Returns a list of items. For example:",
                "#+begin_src http\nGET https://api.example.com/items\n\n> {%",
                "console.log(response.status)",
                "[[https://developer.example.com/docs/createItem][createItem(data)]]",
                "See the full reference for more details.",
            ],
            not_contains: &[
                "\n* API Reference\n\n** Methods",
                "GET https://api.example.com/items > {%",
                "permalink__icon",
            ],
        },
        FixtureCase {
            fixture: "issues--167-partial-selector-inside-code",
            contains: &[
                "The code blocks below use span elements",
                "#+begin_src lean\ndef h1 (x : Nat) : Nat :=\n#+end_src",
                "#+begin_src lean\ndef h2 (x : Nat) : Nat :=\n#+end_src",
                "Both h1 and h2 should appear in the output.",
            ],
            not_contains: &["f-next-next", "f-next-prev"],
        },
        FixtureCase {
            fixture: "issues--168-links-inside-inline-code",
            contains: &[
                "the type ~Nat~ is a common type",
                "single code span: ~List Nat~ should render as ~List Nat~",
                "[[https://example.org/guide][the guide]]",
            ],
            not_contains: &[
                "[[https://example.org/doc/ref/Nat][Nat]]",
                "[[https://example.org/doc/ref/List][List]]",
            ],
        },
        FixtureCase {
            fixture: "issues--159-lean-verso-code-blocks",
            contains: &[
                "standalone ~code.hl.block~ element instead of a ~pre~ wrapper",
                "#+begin_src lean\ndef m : Nat := 1       -- m is a natural number\n#+end_src",
                "Additional text after the code block verifies",
            ],
            not_contains: &["[[https://lean-lang.org/doc/reference", "data-lean-context"],
        },
        FixtureCase {
            fixture: "issues--159-lean-heading-permalink-emoji",
            contains: &[
                "This fixture verifies that heading permalink widgets are removed",
                "** 2.6. Variables and Sections",
                "This section introduces variables and sections in Lean.",
            ],
            not_contains: &["🔗", "Permalink", "permalink-widget"],
        },
        FixtureCase {
            fixture: "issues--159-lean-verso-grouped-blocks",
            contains: &[
                "Verso-style Lean command and output fragments are merged",
                "#+begin_src lean\n#check Nat\nNat : Type\n#check Bool\nBool : Type\n#check Nat → Bool\nNat → Bool : Type\n#+end_src",
                "Text after the example ensures",
            ],
            not_contains: &[
                "#+end_src\n\n#+begin_src lean",
                "hover-container",
                "hover-info",
                "Nat : Type Nat",
            ],
        },
        FixtureCase {
            fixture: "issues--159-lean-verso-empty-line-preserved",
            contains: &[
                "empty Verso code blocks are preserved as blank lines",
                "#+begin_src lean\n#check true\nBool.true : Bool\n\n/- Evaluate -/\n\n#eval 5 * 4\n20\n#+end_src",
                "Trailing prose ensures normal extraction continues.",
            ],
            not_contains: &["#+end_src\n\n#+begin_src lean"],
        },
        FixtureCase {
            fixture: "issues--159-lean-verso-missing-section-gap",
            contains: &[
                "preserve an intentional blank line between adjacent Verso code",
                "#+begin_src lean\ndef b2 : Bool := false\n\n/- Check their types. -/\n\n#check m\nm : Nat\n#+end_src",
                "Trailing prose ensures extraction continues normally.",
            ],
            not_contains: &["#+end_src\n\n#+begin_src lean"],
        },
        FixtureCase {
            fixture: "general--inline-comments-and-link-lists",
            contains: &[
                "The company has officially announced the successor",
                "** Available to order next week",
                "- Gesture Controls",
                "The new model supports high-resolution lossless audio",
                "The new model comes in five color options",
            ],
            not_contains: &[
                "Top comment by",
                "Liked by 28 people",
                "View all comments",
                "Best accessories",
                "affiliate.example",
            ],
        },
        FixtureCase {
            fixture: "general--multi-article-portfolio",
            contains: &[
                "Researcher and developer focused on distributed systems",
                "** Experience",
                "*** Senior Engineer",
                "** Publications",
                "Optimizing Cache Invalidation in Distributed Systems",
            ],
            not_contains: &["Contact me", "View project"],
        },
        FixtureCase {
            fixture: "elements--empty-table",
            contains: &[
                "* Article Title",
                "This is the article content. It has enough words",
            ],
            not_contains: &["|  |  |  |"],
        },
        FixtureCase {
            fixture: "elements--br-between-blocks",
            contains: &[
                "First paragraph with some introductory text.",
                "Second paragraph after a br spacer.",
                "[[https://example.com/image.jpg]]\n\nA sample figure.",
                "Fourth paragraph with trailing br inside.",
                "-----\n\nSeventh paragraph after an hr and br.",
            ],
            not_contains: &["[[https://example.com/image.jpg]] A sample figure."],
        },
        FixtureCase {
            fixture: "general--svg-content-preservation",
            contains: &[
                "Neural networks are the fundamental building block of modern AI",
                "#+begin_export html\n<svg",
                "viewBox=\"0 0 400 100\"",
                "fill=\"Canvas\"",
                "stroke=\"#d97706\"",
                "stroke=\"#16a34a\"",
                "stroke=\"#94a3b8\"",
                "style=\"font-size:14px;font-weight:600\"",
                "2.0",
                "-0.50",
                "#+begin_src python\nmodel = NeuralNetwork(layers=[1, 4, 1])\n#+end_src",
                "This simple model can be trained to approximate many functions",
            ],
            not_contains: &[
                "viewBox=\"0 0 100 300\"",
                "** Further Reading",
                "stroke-zinc-400",
                "fill-orange-500",
            ],
        },
        FixtureCase {
            fixture: "general--svg-external-css-fallback",
            contains: &[
                "Crude oil prices have been steadily climbing",
                "#+begin_export html\n<svg",
                "stroke-opacity=\"0.2\"",
                "stroke=\"currentColor\"",
                "fill=\"none\"",
                "Analysts expect prices to remain elevated",
            ],
            not_contains: &[
                "Further Reading",
                "Related articles",
                "class=\"chart-svg",
                "class=\"gridline",
                "class=\"path-line",
                "svelte-",
            ],
        },
        FixtureCase {
            fixture: "general--wikipedia-ipa-pronunciation",
            contains: &[
                "An *example* (/ɪɡˈzæmpəl/) is a short illustrative instance",
                "Mathematicians often distinguish between examples and counterexamples.",
                "software engineering",
            ],
            not_contains: &["rt-commentedText", "nopopups", "noexcerpt"],
        },
        FixtureCase {
            fixture: "general--wikipedia",
            contains: &[
                "*Obsidian* is a [[https://en.wikipedia.org/wiki/Personal_knowledge_management][personal knowledge management]]",
                "Obsidian is developed by Dynalist Inc.",
                "** History",
                "[fn:2]",
            ],
            not_contains: &[
                "From Wikipedia, the free encyclopedia",
                "Developer(s)",
                "mw-editsection",
                "Navigation menu",
                "Obsidian (software) - Wikipedia",
            ],
        },
        FixtureCase {
            fixture: "issues--169-svg-classname-crash",
            contains: &[
                "This paper discusses algorithms for graph traversal",
                "#+begin_export html\n<svg",
                "viewBox=\"0 0 100 100\"",
                "text-anchor=\"middle\"",
                "x=\"50\" y=\"55\">Node</text>",
                "The algorithm runs in O(n log n) time.",
            ],
            not_contains: &["class=\"diagram\"", "debug-early-backtrace"],
        },
        FixtureCase {
            fixture: "general--developer.mozilla.org-en-US-docs-Web-JavaScript-Reference-Global_Objects-Array",
            contains: &[
                "* Array\n\nThe *Array* object enables storing a collection",
                "** Description",
                "*** Iterative methods",
                "** Constructor",
                "Array.prototype.reduce()",
            ],
            not_contains: &["heading-anchor", "Browser compatibility", "Last modified:"],
        },
        FixtureCase {
            fixture: "elements--empty-p-br",
            contains: &[
                "First section content.",
                "Second section content.",
                "Third section content.",
            ],
            not_contains: &["<br"],
        },
        FixtureCase {
            fixture: "elements--nbsp-handling",
            contains: &[
                "The brief walk home was pleasant. I passed along a pavement full of discarded sweet wrappers and broken toys.",
                "Multiple nbsps should collapse. Single ones between words should stay.",
            ],
            not_contains: &["\u{a0}"],
        },
        FixtureCase {
            fixture: "elements--farsi-zwnj",
            contains: &[
                "** مقدمه",
                "*Bases* یک *افزونه‌ی اصلی*",
                "*نمایی شبیه پایگاه‌داده*",
                "مرتب‌سازی و فیلتر",
                "برنامه‌های سفر",
            ],
            not_contains: &[],
        },
        FixtureCase {
            fixture: "elements--whitespace-newlines",
            contains: &[
                "** Newlines in paragraphs",
                "This is a paragraph with newlines between sentences. Browsers collapse these to spaces.",
                "#+begin_src\n/ip address\nadd address=192.168.88.1/24 interface=bridge1\nadd address=172.16.0.1/24 interface=ether1\n#+end_src",
                "#+begin_src\nThis is a normal pre+code block.\nIt should be preserved as-is.\n  Including indentation.\n#+end_src",
            ],
            not_contains: &["sentences.\nBrowsers"],
        },
        FixtureCase {
            fixture: "elements--figure-content-wrapper",
            contains: &[
                "First paragraph with some content about the topic.",
                "Second paragraph continuing the discussion.",
                "[[https://example.com/diagram.png][A diagram]]",
                "Third paragraph after the image.",
                "Fourth paragraph wrapping up the section.",
                "Author Name",
            ],
            not_contains: &["paragraph-image", "inline-image"],
        },
        FixtureCase {
            fixture: "small-images--svg-icon-viewbox",
            contains: &[
                "* Blog Post Title",
                "This is the main content of the blog post",
                "** First Section",
                "The SVG icons above should be removed",
                "Another paragraph to ensure there is enough content",
            ],
            not_contains: &[
                "[[https://example.org/edit]]",
                "[[https://example.org/source]]",
                "Edit this page",
                "View source",
                "<svg",
            ],
        },
        FixtureCase {
            fixture: "metadata--h1-sibling-byline",
            contains: &[
                "Modern machine learning systems require optimization algorithms",
                "** Background",
                "** Our Approach",
            ],
            not_contains: &[
                "\nMarch 24, 2026\n",
                "\nJane Smith, Research Scientist, and John Doe",
            ],
        },
        FixtureCase {
            fixture: "metadata--date-adjacent-category-link",
            contains: &[
                "*** Introduction",
                "This post covers several important algorithms",
                "*** Solution",
            ],
            not_contains: &[
                "[[https://example-blog.example.com/categories/1][1]]",
                "[[https://example-blog.example.com/tags/libbenchmark][libbenchmark]]",
                "·",
            ],
        },
        FixtureCase {
            fixture: "metadata--295-header-date-above-title",
            contains: &[
                "Example Company is named a leader in the 2026 industry report",
                "The recognition reflects sustained investment",
                "Customers cite measurable gains in review throughput",
            ],
            not_contains: &[
                "[[https://example.com/news/ai-adoption/][AI Adoption]]",
                "A different article you might also like",
                "Yet another related article",
            ],
        },
        FixtureCase {
            fixture: "metadata--295-suggested-post-date",
            contains: &[
                "Example Company is named a leader in the 2026 industry report",
                "The recognition reflects sustained investment",
                "Customers cite measurable gains",
            ],
            not_contains: &[
                ":AUTHOR: Published May 22, 2026",
                "A different article you might also like",
                "Yet another related article",
            ],
        },
        FixtureCase {
            fixture: "issues--285-tag-like-title-escaping",
            contains: &[
                "This is a list of blog posts.",
                "[[https://www.example.com/blog/first-post][An Ordinary First Post]]",
                "[[https://www.example.com/blog/monte-video][Monte<video>: A Real Place]]",
                "Reach us at <hello@example.com>.",
                "[[https://www.example.com/blog/third-post][A Third Post Worth Reading]]",
            ],
            not_contains: &["Montevideo", "</video>"],
        },
        FixtureCase {
            fixture: "metadata--placeholder-values",
            contains: &[
                "Some CMSes leave unresolved template literals",
                "Defuddle should skip over these and fall back to the next valid source.",
                "The real values come from the schema and twitter:description tags.",
            ],
            not_contains: &["{{page.title}}", "{{site.name}}", "{{date}}", ":AUTHOR: . ."],
        },
        FixtureCase {
            fixture: "metadata--author-by-prefix-and-url",
            contains: &[
                "Cloud storage has evolved significantly over the past decade.",
                "** Current Landscape",
                "Today's cloud storage solutions offer features",
            ],
            not_contains: &[
                ":AUTHOR: Dr Jane Smith - https://blog.example.com/",
            ],
        },
        FixtureCase {
            fixture: "metadata--multi-script-values",
            contains: &[
                "人工智能（AI）正在改变我们的世界。",
                "يستكشف هذا المقال كيف يؤثر الذكاء الاصطناعي",
                "日本語でも同様に、人工知能は急速に発展",
            ],
            not_contains: &["{{", "}}"],
        },
        FixtureCase {
            fixture: "issues--196-og-title-brand-name",
            contains: &[
                "We introduce the Darwin Gödel Machine (DGM)",
                "Unlike previous self-improving systems",
                "The system has shown remarkable results",
            ],
            not_contains: &["* Example AI\n:PROPERTIES:"],
        },
        FixtureCase {
            fixture: "metadata--rel-author-in-bio-container",
            contains: &[
                "The body of the article goes here.",
                "Another paragraph follows to reinforce the main content.",
                "One more paragraph to ensure the content is well above the threshold",
            ],
            not_contains: &[
                "A short abstract paragraph",
                "I write about distributed systems",
                "jane.jpg",
            ],
        },
        FixtureCase {
            fixture: "metadata--email-style-header-block",
            contains: &[
                "Date: Wed, 08 Apr 2026",
                "From: Example Team <hello@example.com>",
                "To: Readers",
                "Subject: Announcement Reflection",
                "This note preserves the email-style header",
            ],
            not_contains: &["Subject:\n\nWe are sharing"],
        },
        FixtureCase {
            fixture: "metadata--placeholder-author-and-duplicate-published",
            contains: &[
                "In the last 24 hours, incidents across multiple regions have been recorded.",
                "Defuddle should treat this as no author rather than returning the placeholder verbatim.",
            ],
            not_contains: &[":AUTHOR: . ."],
        },
        FixtureCase {
            fixture: "issues--header-wraps-content",
            contains: &[
                "A few weeks ago, a major cloud provider published a post",
                "** The background",
                "The reference implementation is in JavaScript",
                "** The approach",
                "** The outcome",
            ],
            not_contains: &["Major cloud provider wrote about this topic"],
        },
        FixtureCase {
            fixture: "issues--263-buttons-in-paragraphs",
            contains: &[
                "Given a string ~s~, return /the longest/ /special/ /subsequence/ in ~s~.",
                "*Example 1:*",
                "#+begin_src\nInput: s = \"babad\"",
                "*Constraints:*",
                "- ~1 <= s.length <= 1000~",
            ],
            not_contains: &[
                "Longest Special Sequence - LeetCode",
                "\n* Longest Special Sequence\n\nGiven a string",
                "nav-menu-toggle",
                "aria-haspopup",
            ],
        },
        FixtureCase {
            fixture: "content-patterns--trailing-related-posts",
            contains: &[
                "How Coffee Cools",
                "Most models fit two exponential decay terms",
            ],
            not_contains: &[
                "Maybe there's a pattern here?",
                "The real data wall is billions of years",
                "Why didn't we get GPT-2",
            ],
        },
        FixtureCase {
            fixture: "thin-section-before-see-also",
            contains: &[
                "The concept of global stewardship was first articulated",
                "** Criticism",
                "Scholar [[https://thin-section-before-see-also/wiki/Example_Author][Jane Doe]] has argued",
                "[fn:1]",
                "[fn:1] Doe, Jane (2024). \"Governance and Constraint.\" /Journal of Policy Studies/. 15(3): 42-58.",
            ],
            not_contains: &[
                "** See also",
                "Environmental ethics",
                "Sustainability",
                "Commons",
                "** References",
                "[[#cite_note-1]",
            ],
        },
        FixtureCase {
            fixture: "content-patterns--socket-dev-blog",
            contains: &[
                "We are excited to announce Socket Firewall",
                "** How It Works",
                "Socket Firewall intercepts package installation requests",
                "** Why We Built It",
                "Developers should be able to install packages without worrying",
            ],
            not_contains: &[
                "You're Invited",
                "Subscribe to our newsletter",
                "Ready to block supply chain threats",
                "Supply Chain Security in 2025",
                "Detecting Malicious npm Packages",
            ],
        },
        FixtureCase {
            fixture: "content-patterns--social-counter-link",
            contains: &[
                "Lorem ipsum dolor sit amet",
                "** First Section",
                "Nemo enim ipsam voluptatem quia voluptas",
            ],
            not_contains: &["13 Likes"],
        },
        FixtureCase {
            fixture: "content-patterns--social-engagement-counter",
            contains: &[
                "Plants grow through a process called photosynthesis",
                "The rate of growth depends on several factors including light intensity",
                "Research has shown that talking to plants does not significantly affect",
            ],
            not_contains: &[
                "9 Likes",
                "User A's avatar",
                "User B's avatar",
                "https://example.com/profile/12345-user-a",
                "https://example.com/profile/67890-user-b",
            ],
        },
        FixtureCase {
            fixture: "content-patterns--live-blog-metadata",
            contains: &[
                "Breaking News Live Updates: Major Development",
                "Lorem ipsum dolor sit amet, consectetur adipiscing elit.",
                "** Here's the latest.",
                "Duis aute irure dolor in reprehenderit in voluptate velit esse",
                "** Earlier update",
                "Sed ut perspiciatis unde omnis iste natus error sit voluptatem",
            ],
            not_contains: &[
                "\nPinned\n",
                "Current time in",
                "City A",
                "City B",
                "timezone-widget",
            ],
        },
        FixtureCase {
            fixture: "content-patterns--leading-breadcrumb",
            contains: &[
                "Not a shadowing day or research interview",
                "** Why this industry?",
                "** Getting the job",
            ],
            not_contains: &[
                "[[https://example.com/][Home]]",
                "[[https://example.com/archive][Posts]]",
                "I took a job in an unexpected industry",
            ],
        },
        FixtureCase {
            fixture: "general--back-nav-link",
            contains: &[
                "Sorting algorithms are fundamental to computer science.",
                "Quicksort works by selecting a pivot element",
                "The expected time complexity of Quicksort is O(n log n)",
            ],
            not_contains: &["← back", "../index.html"],
        },
        FixtureCase {
            fixture: "entry-point--js-article-content",
            contains: &[
                "** Section One",
                "** Section Two",
                "#+begin_quote\nProgress is not inevitable. It requires effort and dedication.\n#+end_quote",
                "** Section Three",
            ],
            not_contains: &[
                "Expert News by Example Blog",
                "example.substack.com/embed",
                "See All Newsletters",
                "Views expressed in posts",
                "investment adviser",
                "adviserinfo.sec.gov",
            ],
        },
        FixtureCase {
            fixture: "general--react-streaming-ssr",
            contains: &[
                "\n* Understanding Widget Architecture\n\nModern widget systems",
                "Modern widget systems have evolved significantly",
                "At the core of any widget system is the *render pipeline*.",
                "One of the most important optimizations is *incremental rendering*.",
                "dependency graph",
            ],
            not_contains: &["Loading article", "skeleton", "$RC"],
        },
        FixtureCase {
            fixture: "general--obsidian.md-blog-verify-obsidian-sync-encryption",
            contains: &[
                "On our [[https://obsidian.md][About page]]",
                "#+begin_quote\nWe believe that your thoughts and ideas belong to you",
                "*** How Obsidian Sync works",
                "#+begin_src js\nlet data = await",
                "*2025-09-05 edit:* Updated instructions",
            ],
            not_contains: &["Share this post", "twitter", "reddit.com/submit"],
        },
        FixtureCase {
            fixture: "general--obsidian-publish-cjk",
            contains: &[
                "如果你熟悉 TypeScript 或 CSS",
                "[[https://publish.obsidian.md/help-zh/扩展+Obsidian/第三方插件][第三方插件]]",
                "[[https://docs.obsidian.md][Obsidian 开发者文档]]",
                "[[https://github.com/obsidianmd/obsidian-developer-docs][Obsidian 开发者文档仓库]]",
            ],
            not_contains: &[
                "Links to this page",
                "Interactive graph",
                "Powered by Obsidian Publish",
                "Obsidian 中文帮助 - Obsidian Publish",
            ],
        },
        FixtureCase {
            fixture: "content-patterns--trailing-subscribe-after-footnotes",
            contains: &[
                "This is the first footnote with additional context",
                "This is the second footnote referencing external research",
            ],
            not_contains: &["Subscribe"],
        },
        FixtureCase {
            fixture: "general--trailing-cta-newsletter",
            contains: &[
                "Today we are announcing a major new feature",
                "** What's new",
                "** Technical details",
                "** Getting started",
                "The feature is available today for all users.",
            ],
            not_contains: &[
                "- March 13, 2026",
                "- 4 min",
                "- Share",
                "See how our platform can help your team",
                "Tips, tutorials, and product updates delivered monthly",
            ],
        },
        FixtureCase {
            fixture: "content-patterns--iso-date-and-read-time",
            contains: &["Timeline", "At vero eos et accusamus"],
            not_contains: &[
                "8 min read",
                "[[https://blog.example.com/author/jane/][Jane Smith]]",
            ],
        },
        FixtureCase {
            fixture: "content-patterns--table-of-contents",
            contains: &["Acquire the image", "sha256sum -c --ignore-missing"],
            not_contains: &["[[#1-start-here]", "[[#acquire-the-image]"],
        },
        FixtureCase {
            fixture: "content-patterns--heading-introduced-list",
            contains: &[
                "* sample-plugin",
                "** Features",
                "- Written in ~Lua~",
                "** Install",
                "#+begin_src\nuse 'user/sample-plugin'\n#+end_src",
                "** Configuration",
                "require('sample-plugin').setup({",
                "** Usage",
            ],
            not_contains: &[],
        },
        FixtureCase {
            fixture: "general--appendix-heading",
            contains: &[
                "* Article with Appendix Section",
                "** Introduction",
                "The introduction provides background information",
                "** Results",
                "** Appendix I",
                "** Acknowledgements",
            ],
            not_contains: &[],
        },
        FixtureCase {
            fixture: "general--tailwind-hidden-blog-index",
            contains: &[
                "* Engineering Blog",
                "** Featured",
                "*** [[https://tailwind-hidden-blog-index/blog/scaling-distributed-systems][Scaling Distributed Systems at Acme]]",
                "How we redesigned our message queue infrastructure",
                "*** [[https://tailwind-hidden-blog-index/blog/ml-pipeline-optimization][Optimizing Our ML Pipeline]]",
                "*** [[https://tailwind-hidden-blog-index/blog/design-system-v2][Introducing Our Design System v2]]",
                "*** [[https://tailwind-hidden-blog-index/blog/api-versioning-strategy][Our API Versioning Strategy]]",
            ],
            not_contains: &["not-machine:hidden", "### [", "](https://example.com/"],
        },
        FixtureCase {
            fixture: "issues--pascalcase-section-id-partial-match",
            contains: &[
                "This article wraps each part in a section element",
                "** Opening context",
                "** Why it matters",
                "** The role of things",
                "This section must survive extraction.",
                "** Loops and feedback",
                "** Closing thoughts",
            ],
            not_contains: &[],
        },
        FixtureCase {
            fixture: "content-patterns--content-list-with-internal-links",
            contains: &[
                "Style emerges from consistency, and having a style opens your imagination.",
                "- I use [[https://content-patterns--content-list-with-internal-links/plain-text][plain text files]] for all my writing",
                "- I ask myself [[https://content-patterns--content-list-with-internal-links/annual-review][reflection questions every year]]",
                "Having a style collapses hundreds of future decisions into one",
            ],
            not_contains: &["Personal style guide — Example Blog"],
        },
        FixtureCase {
            fixture: "content-patterns--non-semantic-hero-header",
            contains: &[
                "Industry pioneer John Smith passed away on April 15, 2026.",
                "Smith was a legend whose contributions to the field are too numerous to name",
                "By far, his most significant contributions came in the realm of product design.",
                "Smith was 91 years of age at the time of his passing.",
            ],
            not_contains: &[
                "[[https://example.com/images/hero.jpg]] NEWS",
                "PIONEER RECOGNIZED AT ANNUAL AWARDS CEREMONY",
                "Posted by Jane Reporter on April 20, 2026",
                "ABOUT THE AUTHOR",
                "Managing Editor",
                "MORE NEWS",
                "Company Unveils New Expansion",
                "Publisher Announces Anniversary Edition",
            ],
        },
        FixtureCase {
            fixture: "eyebrow--paragraph-label",
            contains: &[
                "Six months ago, our onboarding completion rate sat at an uncomfortable",
                "Then the growth team ran the numbers.",
                "** What We Changed",
                "** What Happened",
            ],
            not_contains: &["\nBlog post\n"],
        },
        FixtureCase {
            fixture: "eyebrow--linked-category",
            contains: &[
                "At the end of a recent podcast episode",
                "** Parallelism strategies",
                "** Communication patterns",
                "** Practical implications",
            ],
            not_contains: &["AI Research"],
        },
        FixtureCase {
            fixture: "eyebrow--spans-category",
            contains: &[
                "Our latest platform is now generally available",
                "** Built for Teams",
                "** Getting Started",
            ],
            not_contains: &["Product Update"],
        },
        FixtureCase {
            fixture: "general--apnews-link-enhancement",
            contains: &[
                "* Health care subsidies at risk in shutdown",
                "WASHINGTON (AP) — The first caller",
                "[[https://apnews.com/article/aca-credits-health-care-subsidies][under that law]]",
                "[[https://apnews.com/article/aca-credits-subsidies-government-shutdown][subsidies that make insurance affordable]]",
            ],
            not_contains: &["LinkEnhancement"],
        },
        FixtureCase {
            fixture: "headings--fragment-url-not-permalink",
            contains: &[
                "Here are some recommended items from this week",
                "** 1. First Item",
                "** 2. Second Item",
                "** 5. Fifth Item",
                "This heading has a real permalink anchor that should be stripped",
            ],
            not_contains: &[
                "[[https://example.com/items/123][First Item]]",
                "Permanent link",
                "#section-five",
            ],
        },
        FixtureCase {
            fixture: "headings--permalink-title-match",
            contains: &[
                "This is the main content of the blog post.",
                "** First Section",
                "** Second Section",
            ],
            not_contains: &["¶", "Permanent link", "#first-section"],
        },
        FixtureCase {
            fixture: "headings--testid-article-header",
            contains: &[
                "A growing body of research links heavy social media use",
                "** Section heading",
                "[[https://example.com/image.jpg][Photo description]]",
                "Photo credit line.",
                "** Another heading",
            ],
            not_contains: &["data-testid", "<figure"],
        },
        FixtureCase {
            fixture: "issues--sidebar-toggle-checkbox",
            contains: &[
                "The first paragraph introduces the topic",
                "A second paragraph keeps the prose flowing",
                "[[https://sidebar-toggle-checkbox/media/orangepi-storage-migration.svg][Storage migration: SD to NVMe]]",
                "Storage migration: SD to NVMe",
                "A closing paragraph after the diagram explains",
            ],
            not_contains: &[
                "sidebar-checkbox",
                "mm-trigger",
                "[[https://sidebar-toggle-checkbox/][Home]]",
            ],
        },
        FixtureCase {
            fixture: "eyebrow--wrapper-with-icon",
            contains: &[
                "The launch provider could not celebrate the achievement for long.",
                "A statement issued by the satellite operator later that day",
                "** What went wrong",
                "This is, in other words, a mission that will be remembered",
            ],
            not_contains: &[
                "\nOff-nominal\n",
                "\n* Launch Anomaly Disrupts Otherwise Successful Mission\n\nThe launch provider",
            ],
        },
        FixtureCase {
            fixture: "content-patterns--card-grid-stripped-headings",
            contains: &[
                "Spacecraft plumbing systems must function in microgravity",
                "Engineers have developed specialized pumps and containment systems",
                "Waste management systems have seen the most dramatic improvements",
                "\"We have come a long way from the early days,\" said lead engineer Dr. Park.",
            ],
            not_contains: &[
                "New Ion Thruster Breaks Efficiency Record",
                "Mars Sample Return Mission Gets Green Light",
                "Commercial Space Station Module Passes Critical Review",
                "Alex Johnson",
                "Maria Lopez",
                "Chris Park",
            ],
        },
        FixtureCase {
            fixture: "footnotes--span-fnref-colon",
            contains: &[
                "calibration",
                "[fn:1]",
                "[fn:1] Calibration details are described in the reference manual.",
                "[fn:3] Experimental validation was performed under controlled conditions.",
            ],
            not_contains: &["footnote-backref", "fnref:calibration"],
        },
        FixtureCase {
            fixture: "footnotes--maggieappleton.com-xanadu-patterns",
            contains: &[
                "* The Pattern Language of Project Xanadu",
                ":SITE: maggieappleton.com",
                ":PUBLISHED: 2024-05-24T02:20:06+00:00",
                "When it comes to visionary, before-its-time software",
                "Xanadu was a hypothetical hypertext system. [fn:1]",
                "[fn:1] The word hypothetical is a bit strong",
                "[fn:5] Using [[https://en.wikipedia.org/wiki/Cave_of_La_Pasiega][La Pasiega Cave]]",
                "* Draft in Progress",
                "The quality of writing below this point is haphazard",
                "Roam Research, Obsidian, Logseq, and Innos",
            ],
            not_contains: &[
                ":AUTHOR: grand theft eigenvalue",
                ":SITE: grand theft eigenvalue",
                "Xanadu was a hypothetical hypertext system. The word hypothetical",
                "Mentions around the web",
                "Project Xanadu self-assembly",
                "Show 2 more",
                "*** [[https://en.wikipedia.org/wiki/A_Pattern_Language][A Pattern Language]]",
                "The book outlined a number of solutions for architecture",
            ],
        },
        FixtureCase {
            fixture: "footnotes--google-docs-ftnt",
            contains: &[
                "29 more",
                "[fn:1]",
                "[fn:1] These exploits target a testing harness",
                "[fn:2] This testing was conducted in isolated environments",
            ],
            not_contains: &["#ftnt_ref", "*** Footnotes"],
        },
        FixtureCase {
            fixture: "footnotes--wp-block-footnotes",
            contains: &[
                "[fn:1]",
                "[fn:2]",
                "[fn:3] See the [[https://example.com/report][full comparison report]]",
            ],
            not_contains: &["wp-block-footnotes", "Jump to footnote reference"],
        },
        FixtureCase {
            fixture: "footnotes--easy-footnotes-wrapper",
            contains: &[
                "peaceful traders",
                "[fn:1]",
                "[fn:1] A. Smith, /A History of Naval Power Before the Great War/",
                "[[https://example.com/soldiers-and-silver][Soldiers & Silver]]",
            ],
            not_contains: &["easy-footnote", "easy-footnote-bottom"],
        },
        FixtureCase {
            fixture: "footnotes--word-ftn-ftnref",
            contains: &[
                "Action Plan [fn:1],",
                "[fn:1] [[https://example.org/action-plan/][https://example.org/action-plan/]]",
                "[fn:2] Country A, Country B, Country C.",
            ],
            not_contains: &["#_ftnref", "//6CB562CC"],
        },
        FixtureCase {
            fixture: "footnotes--numeric-anchor-id",
            contains: &[
                "Dietary guidelines were examined in the first study [fn:1]",
                "subsequent research [fn:2] and a meta-analysis [fn:3]",
                "[fn:1] A. Author, B. Researcher.",
                "[fn:2] C. Scholar, D. Investigator.",
                "[fn:3] E. Analyst, F. Reviewer.",
            ],
            not_contains: &["[[#ref1]", "reference-number", "** References"],
        },
        FixtureCase {
            fixture: "footnotes--named-anchor",
            contains: &[
                "sacred architecture for centuries. [fn:1]",
                "semicircular apse. [fn:2] The colonnaded nave",
                "their original splendour. [fn:3]",
                "centuries of accretion. [fn:4]",
                "[fn:1] For a full account",
                "[fn:4] Historical Essay on Architecture",
            ],
            not_contains: &["FnAnchor_", "Footnote_", "[[#Footnote"],
        },
        FixtureCase {
            fixture: "footnotes--p-class-footnote",
            contains: &[
                "This pattern language [fn:1] describes a new approach.",
                "basic concepts [fn:2] behind the approach.",
                "[fn:1] The structure of this article was inspired",
                "[fn:2] These concepts were first described",
            ],
            not_contains: &["<sup>1", "class=\"footnote\""],
        },
        FixtureCase {
            fixture: "footnotes--inline-footnote-span",
            contains: &[
                "The field has drifted far from its roots. [fn:1]",
                "Philosophy is often viewed as a science, which leads to misalignment. [fn:2]",
                "facts and values are orthogonal. [fn:3]",
                "[fn:1] You can pick any recent paper",
                "[fn:2] The most common areas where this occurs are [[https://en.wikipedia.org/wiki/Analytic_philosophy][analytic philosophy]]",
                "[fn:3] Orthogonal means they run independent",
            ],
            not_contains: &["inline-footnote", "footnoteContent", "display:none"],
        },
        FixtureCase {
            fixture: "footnotes--bold-sup-trailing",
            contains: &[
                "The protocol handles mobility transparently. [fn:1]",
                "prevent session hijacking. [fn:2]",
                "There are several candidate protocols. [fn:3]",
                "*Update 2024-06-01:* A follow-up discussion",
                "[fn:1] *Note 2024-01-15:* This also works",
                "[fn:3] *Note 2024-03-20:* Besides the primary candidate",
            ],
            not_contains: &["<sup>1", "<sup>2", "<sup>3"],
        },
        FixtureCase {
            fixture: "footnotes--hr-sup-numbered",
            contains: &[
                "The first technique is widely used. [fn:1]",
                "The second technique has trade-offs. [fn:2]",
                "Both approaches have merit.",
                "[fn:1] This is the first footnote",
                "[fn:2] This is the second footnote",
            ],
            not_contains: &["-----", "<sup>1", "<sup>2"],
        },
        FixtureCase {
            fixture: "footnotes--hr-strong-numbered",
            contains: &[
                "The first concept is well-established. [fn:1]",
                "The second concept is more nuanced. [fn:2]",
                "Concluding thoughts.",
                "[fn:1] First footnote explanation.",
                "- Supporting point A.",
                "- Supporting point B.",
                "[fn:2] Second footnote with additional context.",
            ],
            not_contains: &["-----", "<strong>1", "<strong>2"],
        },
        FixtureCase {
            fixture: "footnotes--hr-continuation",
            contains: &[
                "The first approach has notable trade-offs. [fn:1]",
                "The second approach handles scale differently. [fn:2]",
                "[fn:1] The first approach introduces complexity:",
                "- Higher operational overhead.",
                "Despite this, teams often prefer it for its flexibility.",
                "[fn:2] The second approach favors simplicity",
                "It performs well under load but limits customization.",
            ],
            not_contains: &["-----", "<strong>1", "<strong>2"],
        },
        FixtureCase {
            fixture: "footnotes--aside-ol-start",
            contains: &[
                "We built a new library. [fn:1]",
                "[[https://example.com/docs][check it out here]]",
                "** What is property-based testing?",
                "The main [fn:2] benefits are:",
                "- Automatic shrinking to minimal failing examples. [fn:3]",
                "[fn:1] It's a philosophy joke.",
                "[fn:2] There are other benefits too",
                "[fn:3] Shrinking finds the smallest input",
                "[[https://example.com/shrinking][this post]]",
            ],
            not_contains: &["aside-link", "<aside", "<ol start"],
        },
        FixtureCase {
            fixture: "footnotes--sidenote-inline-with-list",
            contains: &[
                "\n* Sample post title",
                "roundness [fn:1] and some more text",
                "influence [fn:2] on the broader industry",
                "[fn:1] One of the ugliest roundness examples is the YouTube UI",
                "[fn:2] That's to say, contagious from inwards",
            ],
            not_contains: &[
                "sidenote-number",
                "footnote-reference",
                "footnote-reference-1",
                "footnote-reference-2",
            ],
        },
        FixtureCase {
            fixture: "footnotes--pulldown-cmark-footnote-definition",
            contains: &[
                "Example bit-shift analysis: a common simplification is to replace ~x & \\~0~ with ~x~ [fn:1].",
                "Other paragraph content follows",
                "Thanks to everyone for reading.",
                "[fn:1] Possibly with masking of the top bit",
                "~x & 0x7fff..ffff~",
            ],
            not_contains: &["-----", "footnote-definition-label"],
        },
        FixtureCase {
            fixture: "footnotes--orgmode-css-sidenotes",
            contains: &[
                "First paragraph with a sidenote reference. [fn:1] More text continues here.",
                "Second paragraph with another reference. [fn:2]",
                "Third paragraph with a final reference. [fn:3]",
                "[fn:1] This is the first footnote content.",
                "[fn:2] This is the second footnote with more detail about the topic.",
                "[fn:3] A reader suggested an alternative approach",
            ],
            not_contains: &["footref-toggle", "\n* Sidenotes", "class=\"sidenote\""],
        },
        FixtureCase {
            fixture: "footnotes--hidden-section",
            contains: &[
                "We're lucky to have so many good RSS readers that cut through this nonsense. [fn:1]",
                "More article text here to ensure content is detected.",
                "[fn:1] [[https://netnewswire.com][NetNewsWire]]",
                "[[https://reederapp.com/classic/][Reeder]]",
            ],
            not_contains: &["data-footnote-backref", "Back to reference", "↩"],
        },
        FixtureCase {
            fixture: "footnotes--hidden-aside-data-definition",
            contains: &[
                "six threads to follow. [fn:1]",
                "smattering of references from the other entries. [fn:2]",
                "welcome addition to the lore. [fn:3]",
                "[fn:1] By my count: 1) Alice and Bob.",
                "[fn:2] It took me a while to understand",
                "[[https://example.com/wiki/The_Leader][the Leader's]]",
                "[fn:3] I'm glad to see the old guard back",
            ],
            not_contains: &["fna-ref", "asterisk-ref", "<aside", "display:none"],
        },
        FixtureCase {
            fixture: "footnotes--heading-notes",
            contains: &[
                "The first observation is notable. [fn:1]",
                "The second finding contradicts it. [fn:2]",
                "Both deserve further study.",
                "[fn:1] First note with supporting detail.",
                "[fn:2] Second note with additional context.",
            ],
            not_contains: &["** Notes", "-----"],
        },
        FixtureCase {
            fixture: "footnotes--nested-prose",
            contains: &[
                "The first claim is well-established. [fn:1]",
                "The second claim is more contested. [fn:2]",
                "The conclusion follows from both claims.",
                "[fn:1] Supporting evidence for the first claim.",
                "[fn:2] Counterarguments and rebuttal for the second claim.",
            ],
            not_contains: &["-----"],
        },
        FixtureCase {
            fixture: "footnotes--child-anchor-id",
            contains: &[
                "foundational sorting algorithm [fn:1] that became widely used",
                "partitioning technique [fn:2] described in an earlier paper",
                "programs [fn:3]. This built on prior research",
                "formal semantics [fn:4].",
                "publications [fn:3] [fn:5].",
                "**** References and notes",
                "[fn:1] A. Author: /The Sorting Algorithm/",
                "[fn:5] A. Author: /Communicating Sequential Processes/",
            ],
            not_contains: &["[[#r1]", "[[#r3]"],
        },
        FixtureCase {
            fixture: "footnotes--external-labeled-section",
            contains: &[
                "- /Higher resolution/. The system now processes images at up to 4096 pixels, more than three times the previous limit. [fn:1]",
                "- /Better accuracy/. Internal testing showed significant improvements in accuracy. [fn:2]",
                "Users can start using these features immediately.",
                "[fn:1] This is a [[https://example.com/docs/vision][configuration change]]",
                "- Benchmark A: scores reflect revised grading methodology.",
                "- Benchmark B: memorization screens flag a subset of problems.",
                "[fn:2] Based on internal evaluations using standardized test harnesses.",
            ],
            not_contains: &["PostDetail__abc123__footnotes", "** Footnotes"],
        },
        FixtureCase {
            fixture: "footnotes--wikipedia-references",
            contains: &[
                "first described in the early twentieth century by several independent scholars. [fn:1]",
                "new applications were discovered in engineering and applied mathematics. [fn:2]",
                "** Applications",
                "materials science. [fn:3] Recent work has extended its use to computational biology. [fn:4]",
                "[fn:1] Author, A. (2005). \"Introduction to placeholder topics.\" /Journal of Examples/",
                "[fn:4] Smith, D.; Jones, E. (2020). \"Computational biology frameworks.\" /Biology Quarterly/",
            ],
            not_contains: &["cite-bracket", "mw-cite-backlink", "** See also", "** References"],
        },
        FixtureCase {
            fixture: "issues--218-footnote-wrapper-text-lost",
            contains: &[
                "The word before the footnote is inside the same wrapper span as the reference. [fn:1] This continues after the footnote.",
                "Another example where the wrapped word should be preserved. [fn:2] More text follows here.",
                "[fn:1] First footnote content.",
                "[fn:2] Second footnote content.",
            ],
            not_contains: &["wrap-inline", "footnote-ref", "footnotes-list"],
        },
        FixtureCase {
            fixture: "issues--296-oreilly-noteref-footnotes",
            contains: &[
                "large values can occasionally lose precision when widened to another numeric type. [fn:1] The compiler applies",
                "define them otherwise. [fn:2] Most code relies",
                "#+begin_src",
                "Swap (ref x, ref y);",
                "[[https://www.example.com/library/view/sample-book/ch03.html#generics][\"Generics\"]]",
                "[fn:1] A minor caveat is that very large values lose some precision",
                "[fn:2] An exception is when calling certain interop methods",
            ],
            not_contains: &["data-type=\"noteref\"", "footnotes-heading"],
        },
        FixtureCase {
            fixture: "issues--280-texinfo-footnotes",
            contains: &[
                "** 1 Introduction",
                "files [fn:1] in ~/usr/local/man/man1~:",
                "#+begin_src\na2p.1\nctags.1\nemacs.1",
                "home directory [fn:2], especially when coupled with version control systems [fn:3].",
                "[fn:1] As of an ancient release. These are now old versions but the example still holds valid.",
                "[fn:2] [[https://www.example.com/news/using-widget-to-manage-your-dotfiles.html][https://www.example.com/news/using-widget-to-manage-your-dotfiles.html]]",
                "[fn:3] [[https://lists.example.com/archive/html/info-widget/msg00000.html][https://lists.example.com/archive/html/info-widget/msg00000.html]]",
            ],
            not_contains: &["footnote-body-heading", "FOOT1", "DOCF1", "nav-panel"],
        },
        FixtureCase {
            fixture: "issues--120-dhammatalks-footnotes",
            contains: &[
                "\n* The Shorter Craving-Destruction Discourse Cūḷa Taṇhāsaṅkhaya Sutta (MN 37)",
                "foremost among devas & human beings?” [fn:1]",
                "‘All dhammas are unworthy of adherence.’ [fn:2] Having heard",
                "feeling. [fn:3] As he remains focused",
                "Kosiya, [fn:4] how did the Blessed One describe",
                "See also: [[https://120-dhammatalks-footnotes/suttas/SN/SN27_1.html][SN 27:1–10]]",
                "[fn:1] According to [[https://120-dhammatalks-footnotes/suttas/AN/AN7_58.html][AN 7:58]]",
                "[fn:2] The Commentary identifies “all dhammas” here",
                "As [[https://120-dhammatalks-footnotes/suttas/AN/AN9_36.html][AN 9:36]] shows",
                "[fn:7] The Commentary says that Sakka is Ven. Mahā Moggallāna’s fellow",
            ],
            not_contains: &["notetitle", "mn37note01", "class=\"fn\"", "navWrapper"],
        },
        FixtureCase {
            fixture: "footnotes--no-false-positive-equation-refs",
            contains: &[
                "[[#thm-residue][1]]",
                "[[#bernstein-exp][2]]",
                "Theorem 1 (Residue Theorem)",
            ],
            not_contains: &["[fn:"],
        },
        FixtureCase {
            fixture: "elements--bootstrap-alerts",
            contains: &[
                "#+begin_quote\n[!info] Important",
                "[!warning] Warning",
                "[!danger] Danger",
                "[!success] Success",
                "Here is some content after the alerts.",
            ],
            not_contains: &["alert-heading", "alert-title"],
        },
        FixtureCase {
            fixture: "elements--hugo-admonitions",
            contains: &[
                "[!info] Note",
                "This is an informational note about the topic.",
                "[!warning] Warning",
                "[!danger] Danger",
                "[!tip] Helpful tip",
            ],
            not_contains: &["details-icon", "admonition-title"],
        },
        FixtureCase {
            fixture: "callouts--obsidian-publish-callouts",
            contains: &[
                "[!info] A callout title",
                "Here is the callout *body* content.",
                "[!question]- Is this foldable?",
                "Yes, the content is hidden when collapsed.",
                "[!note] Note",
            ],
            not_contains: &["callout-fold", "display: none"],
        },
        FixtureCase {
            fixture: "math--raw-latex",
            contains: &[
                "computes $h_{i}^{\\ell+1} = \\text{Attention}",
                "$$\n\\mathcal{L}(\\theta)",
                "The backslash delimiters work too: $E = mc^2$ is inline",
                "$$\nF = ma\n$$",
                "This costs $100 per unit, which is not math.",
                "This $\\alpha$ should not be touched inside pre tags.",
            ],
            not_contains: &[],
        },
        FixtureCase {
            fixture: "math--mathjax-tex-scripts",
            contains: &[
                "energy differences $E_u$ and $E_l$",
                "$$\nf_u = D+\\gamma B",
                "$$\nB= \\frac{1}{\\sqrt{3}\\gamma}",
                "Here, $\\gamma = 28$ GHz/T",
            ],
            not_contains: &[],
        },
        FixtureCase {
            fixture: "math--katex",
            contains: &[
                "\\displaystyle \\frac{1}{\\Bigl(\\sqrt{\\phi \\sqrt{5}}",
                "$k_{n+1} = n^2 + k_n^2 - k_{n-1}$",
                "\\begin{aligned}",
                "\\begin{pmatrix}",
            ],
            not_contains: &["katex-html", "aria-hidden"],
        },
        FixtureCase {
            fixture: "math--katex-data-math",
            contains: &[
                "$\\forall x (M(x) \\rightarrow D(x))$",
                "For all $x$, if $x$ is a man",
                "$$\nx = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}\n$$",
            ],
            not_contains: &["katex-html", "data-math"],
        },
        FixtureCase {
            fixture: "math--temml",
            contains: &[
                "Inline fraction: $\\frac{a}{b}$ and square root: $\\sqrt{x^2+y^2}$.",
                "Cube root: $\\sqrt[3]{8}$",
                "$$\nE = mc^2\n$$",
                "| $\\sum$ ~\\sum~ | $\\int$ ~\\int~ | $\\prod$ ~\\prod~ | $\\oint$ ~\\oint~ |",
                "Greek letters: $\\alpha$, $\\beta$, $\\gamma$, $\\Omega$",
                "Relation: $a \\leq b$, not equal: $x \\neq y$",
            ],
            not_contains: &["Inline fraction: $ab$", "$E=mc2$"],
        },
        FixtureCase {
            fixture: "math--small-equation-image",
            contains: &[
                "$$\n\\operatorname{fn}(x,y) = \\exp(x) - \\log(y)\n$$",
                "$$\n\\begin{align*} \\exp(z)",
                "\\log(z) &\\mapsto \\operatorname{fn}(1,\\exp(\\operatorname{fn}(1,z))) \\end{align*}\n$$",
            ],
            not_contains: &["[[https://example.com/inline-eq.svg", "block-eq.svg"],
        },
        FixtureCase {
            fixture: "math--latex-image-services",
            contains: &[
                "Inline fraction: $\\frac{a}{b}$.",
                "Sum notation: $\\sum_{i=1}^{n}a_i$.",
                "Greek letters: $\\alpha+\\beta$.",
                "Path-based service: $\\frac{m}{v}$.",
            ],
            not_contains: &[
                "latex.codecogs.com",
                "chart.apis.google.com",
                "[[https://example.com/formula",
            ],
        },
        FixtureCase {
            fixture: "math--mathjax-svg",
            contains: &[
                "We define the reward as $r_{\\text{perf}}$.",
                "$$\nr_{PARL} \\left(x , y\\right) = \\lambda_{1} \\cdot r_{\\text{parallel}} + \\lambda_{2} \\cdot r_{\\text{finish}} + r_{\\text{perf}} \\left(x , y\\right)\n$$",
            ],
            not_contains: &["$rperf$", "rPARL(x,y)=λ1⋅rparallel"],
        },
        FixtureCase {
            fixture: "math--mathjax-tagged-equation",
            contains: &[
                "produced by ~\\tag{}~",
                "$$\na = b + c ( \\star )\n$$",
                "$$\n\\begin{aligned}x & = & y \\\\ u & = & v\\end{aligned}\n$$",
            ],
            not_contains: &["(⋆)a=b+c", "$$\nx=yu=v\n$$"],
        },
        FixtureCase {
            fixture: "math--wikipedia-mathml",
            contains: &[
                "$ax^{2}+bx+c=0$",
                "x={\\frac {-b\\pm {\\sqrt {b^{2}-4ac}}}{2a}}",
                "$b^{2}-4ac$",
            ],
            not_contains: &["quad.svg", "mwe-math-fallback", "alttext"],
        },
        FixtureCase {
            fixture: "math--mathjax-chtml-no-assistive",
            contains: &[
                "$$\nz = \\frac{1}{n} \\sum_{i = 1}^{n} x_{i}\n$$",
                "$$\n\\sigma_{group} = \\frac{\\sigma}{\\sqrt{n}}\n$$",
                "$$\np_{group} = \\frac{1}{\\sigma_{group}^{2}}\n$$",
                "Gaussian distribution $N \\left(s , \\sigma_{i}^{2}\\right)$ with a fixed mean.",
                "This lazily-loaded formula was never rendered and has no recoverable content, so it is dropped:",
                "The surrounding prose continues after the dropped placeholder",
            ],
            not_contains: &["| 𝑧 =", "mjx-lazy", "CtxtMenu_Attached_0"],
        },
        FixtureCase {
            fixture: "math--mathjax",
            contains: &[
                "$a \\neq 0$",
                "there are two solutions to $a x^{2} + b x + c = 0$ and they are",
                "x = \\frac{- b \\pm \\sqrt{b^{2} - 4 a c}}{2 a}",
                "$$\n\\begin{aligned}\\overset{\\cdot}{x} & = \\sigma \\left(y - x\\right)",
                "\\left(\\sum_{k = 1}^{n} a_{k} b_{k}\\right)",
                "\\mathbf{V}_{1} \\times \\mathbf{V}_{2}",
                "$k$ heads when flipping $n$ coins is:",
                "\\overset{\\rightarrow}{\\mathbf{B}}",
                "$\\sqrt{3 x - 1} + \\left(1 + x\\right)^{2}$",
            ],
            not_contains: &[
                "there are two solutions to and they are",
                "heads when flipping coins is",
                "CtxtMenu_Attached_0",
            ],
        },
        FixtureCase {
            fixture: "math--katex-centraliser",
            contains: &[
                "[[https://katex-centraliser/centraliser.pdf][PDF version]]",
                "**** Theorem",
                "/If $g$ is an element of $G=S_n$",
                "$$\n\\begin{aligned}\nC_G(g)=\\langle g\\rangle\\iff",
                "Cycles of coprime length will have unequal lengths unless they are 1-cycles.",
            ],
            not_contains: &["katex-html", "aria-hidden", "copy-tex"],
        },
    ];
    let content_only_fixtures = [
        "elements--srcset-normalization",
        "elements--base64-placeholder-removal",
        "elements--svg-placeholder-lazy-image",
        "codeblocks--react-syntax-highlighter-linenums",
        "codeblocks--chatgpt-codemirror",
        "codeblocks--rockthejvm.com-articles-kotlin-101-type-classes",
        "content-patterns--code-block-boilerplate-and-trailing-section",
        "issues--221-nextjs-noscript-images",
        "issues--227-noscript-lazy-images",
        "elements--image-dedup",
        "elements--lightbox-image-dedup",
        "codeblocks--hljs-header",
        "math--katex-data-math",
        "math--wikipedia-mathml",
        "math--mathjax-chtml-no-assistive",
        "math--katex-centraliser",
        "table-layout--single-column",
        "table-layout--blogger-two-column",
        "elements--data-table",
        "elements--br-between-blocks",
        "elements--figure-content-wrapper",
        "issues--300-nested-layout-tables",
        "table-layout--peripheral-tables",
        "issues--284-table-cell-header-scoring",
        "issues--218-footnote-wrapper-text-lost",
        "issues--120-dhammatalks-footnotes",
        "issues--280-texinfo-footnotes",
        "issues--296-oreilly-noteref-footnotes",
        "issues--217-writerside-docs",
        "issues--167-partial-selector-inside-code",
        "issues--168-links-inside-inline-code",
        "issues--159-lean-verso-code-blocks",
        "issues--159-lean-heading-permalink-emoji",
        "issues--159-lean-verso-grouped-blocks",
        "issues--159-lean-verso-empty-line-preserved",
        "issues--159-lean-verso-missing-section-gap",
        "issues--131-category-links",
        "issues--sidebar-toggle-checkbox",
        "headings--fragment-url-not-permalink",
        "headings--permalink-title-match",
        "headings--testid-article-header",
        "small-images--svg-icon-viewbox",
        "issues--162-aria-hidden-main-content",
        "issues--106-menu-id",
        "standardize--span-data-as-paragraph",
        "elements--empty-table",
        "content-patterns--live-blog-metadata",
        "content-patterns--socket-dev-blog",
    ];

    for case in cases {
        let html_path = defuddle_dir
            .join("tests")
            .join("fixtures")
            .join(format!("{}.html", case.fixture));
        let expected_path = defuddle_dir
            .join("tests")
            .join("expected")
            .join(format!("{}.md", case.fixture));
        let html = std::fs::read_to_string(&html_path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", html_path.display()));
        let expected = read_expected_metadata(&expected_path);
        let url = fixture_url_override(case.fixture).or_else(|| fixture_url(&html));
        let output = parse_html_to_org(
            &html,
            DefuddleOptions {
                url,
                include_images: true,
                remove_small_images: true,
                content_selector: None,
                include_replies: IncludeReplies::Extractors,
                remove_hidden_elements: true,
                remove_exact_selectors: true,
                remove_partial_selectors: true,
                remove_content_patterns: true,
                remove_low_scoring: true,
                standardize: true,
                debug: false,
                profile: false,
                frontmatter: false,
                markdown: false,
                separate_markdown: false,
            },
        )
        .unwrap_or_else(|err| panic!("failed to parse {}: {err}", case.fixture));

        if !content_only_fixtures.contains(&case.fixture) {
            assert_eq!(
                output.title, expected.title,
                "fixture {} title",
                case.fixture
            );
            assert_eq!(
                output.author, expected.author,
                "fixture {} author",
                case.fixture
            );
            assert_eq!(output.site, expected.site, "fixture {} site", case.fixture);
            assert_eq!(
                output.published, expected.published,
                "fixture {} published",
                case.fixture
            );
        }
        for expected_text in case.contains {
            assert!(
                output.org.contains(expected_text),
                "fixture {} missing expected text {:?}\n{}",
                case.fixture,
                expected_text,
                output.org
            );
        }
        for unexpected_text in case.not_contains {
            assert!(
                !output.org.contains(unexpected_text),
                "fixture {} included unexpected text {:?}\n{}",
                case.fixture,
                unexpected_text,
                output.org
            );
        }
        assert!(
            output.word_count > 5,
            "fixture {} produced too little text",
            case.fixture
        );
    }
}

fn read_expected_metadata(path: &std::path::Path) -> ExpectedMetadata {
    let markdown = std::fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    let json = markdown
        .strip_prefix("```json\n")
        .and_then(|rest| rest.split_once("\n```\n"))
        .map(|(json, _)| json)
        .unwrap_or_else(|| {
            panic!(
                "{} does not start with a JSON metadata block",
                path.display()
            )
        });
    serde_json::from_str(json)
        .unwrap_or_else(|err| panic!("failed to parse metadata in {}: {err}", path.display()))
}

fn fixture_url(html: &str) -> Option<String> {
    Regex::new(r#"<!--\s*\{\s*"url"\s*:\s*"([^"]+)""#)
        .ok()?
        .captures(html)?
        .get(1)
        .map(|m| m.as_str().to_string())
}

fn fixture_url_override(fixture: &str) -> Option<String> {
    match fixture {
        "content-patterns--content-list-with-internal-links" => {
            Some("https://content-patterns--content-list-with-internal-links/".to_string())
        }
        "thin-section-before-see-also" => {
            Some("https://thin-section-before-see-also/wiki/Example_Concept".to_string())
        }
        "issues--131-category-links" => Some("https://131-category-links/".to_string()),
        "issues--sidebar-toggle-checkbox" => Some("https://sidebar-toggle-checkbox/".to_string()),
        "general--tailwind-hidden-blog-index" => {
            Some("https://tailwind-hidden-blog-index/blog".to_string())
        }
        "issues--120-dhammatalks-footnotes" => {
            Some("https://120-dhammatalks-footnotes/".to_string())
        }
        "math--katex-centraliser" => Some("https://katex-centraliser/".to_string()),
        _ => None,
    }
}
