#import "/lib/lib.typ": *;
// base_path 定义在 contents.typ。lib.typ 的 `import ...: *` 不会把它的
// import 再导出一遍,所以这里要单独引一次(同 toc.typ)。
#import "contents.typ": *;

// 顶栏是**独立编译出来的一页**(lib/header.html),运行时由 header.js 的
// `<site-header>` 抓进 shadow root 渲染 —— 和左栏的 lib/toc.typ /
// `<toc-component>` 是同一套路。顶栏是全站的东西,不该寄生在「本页目录」
// 这个组件里,后者只管 `nav.outline`。
//
// with_header: false —— 这一页自己不要顶栏,否则 <site-header> 会去抓
// 自己。(toc.typ 同理。)
#show: template.with(toc: false, with_outline: false, with_header: false);

#context {
    html_elem("header", attrs: (class: "site-header"), {
        html_elem("div", attrs: (class: "header-left"), {
            // 三个按钮都带 aria-label:☰ 本身是图形,不给名字读屏只会念「按钮」。
            html_elem("button", attrs: (class: "toc-toggle", aria-label: "全站目录"))[☰]
            html_elem("a", attrs: (class: "site-title", href: base_path + "/index.html"))[rust2genshin]
            // 两个分区。选中态是运行时算的(见 header.js),这里只管给出
            // `data-tab` 和各自的落点;两处都要改时别只改一边。
            html_elem("nav", attrs: (class: "site-tabs", aria-label: "分区"), {
                html_elem("a", attrs: (
                    class: "site-tab", data-tab: "docs",
                    href: base_path + "/index.html",
                ))[文档]
                html_elem("a", attrs: (
                    class: "site-tab", data-tab: "play",
                    href: base_path + "/play",
                ))[摆弄]
            })
        })
        html_elem("div", attrs: (class: "header-right"), {
            // 站点在 GitHub 上的仓库。图标走 CSS mask,所以这里内容留空 ——
            // 和 theme-toggle 同理,见 lib/css/header.css。
            html_elem("a", attrs: (
                class: "github-link",
                href: "https://github.com/mzverse/rust2genshin",
                aria-label: "GitHub 主页",
            ))[]
            // theme-toggle 那条 aria-label 是给 JS 前的一版静态文案,
            // header.js 建好 shadow 后会按**目标**状态(而不是当前状态)改写它。
            html_elem("button", attrs: (class: "theme-toggle", aria-label: "切换主题"))[]
            html_elem("button", attrs: (class: "outline-toggle", aria-label: "本页目录"))[☰]
        })
    })
}