#import "contents.typ": *;

#let this-path = state("this-path");

// with_header / with_outline / toc 是三道独立的开关,各自对应一个运行时
// 注入的自定义元素:<site-header>(顶栏)、nav.outline(本页目录)、
// <toc-component>(左栏全站目录)。凡是「自己会被别人抓去渲染」的页面
// (toc.typ / header.typ)都必须把自己那道关掉,否则元素会去抓自己。
#let template(
    con,
    toc: true,
    with_outline: true,
    with_header: true,
    title: none,
) = context {
    set text(lang: "zh", font: "Microsoft YaHei", size: 12pt);
    show link: if target() == "html" {
        it => {
            let dest = it.dest;
            if type(dest) == str and dest.starts-with("/") {
                html.a(href: base_path + dest, it.body)
            } else {
                it
            }
        }
    } else {
        it => {
            if type(it.dest) == label {
                [link\<#str(it.dest)\>\[#it.body\]]
            } else {
                it
            }
        }
    };
    show raw: set text(font: ("Consolas", "Microsoft YaHei"));
    context html_elem("html", attrs: (lang: text.lang), {
        import "style.typ";
        show: style.template-style;
        import "math_render.typ";
        show: math_render.template;
        import "code_block.typ";
        show: code_block.template;
        html_elem("head", {
            html.meta(charset: "utf-8");
            html.meta(name:"viewport", content: "width=device-width, initial-scale=1.0");
            html.base(href: base_path + this-path.get());
            if document.title != none {
                html_elem("title", contentToString(document.title));
            }
            import_script("/lib/js/theme.js");
            import_script("/lib/js/style.js");
            import_style("/lib/css/theme.css");
            import_style("/lib/css/style.css");
            import_style("/lib/css/template.css");
            include math_render;
            if toc {
                import_script("/lib/js/toc.js");
            }
            if with_header {
                import_script("/lib/js/header.js");
            }
            for x in query(<custom-element>).dedup() {
                (x.value)();
            }
        });
        if toc {
            html_elem("toc-component")[];
        }
        // 顶栏得排在 main 之前 —— 它是 body grid 的第 1 行,而正文的
        // 自动放置在它之后(header.css 里那条 grid-row: 1 是兜底)。
        if with_header {
            html_elem("site-header")[];
        }
        import "outline.typ";
        let page_style = if with_outline { outline.template } else { x => x };
        html_elem("main")[
            #show: page_style;
            #if document.title != none {
                html_elem("h1", document.title)
            }
            #con;
            #if toc {
                html.div(style: "height: calc(100vh - 5em);")
            }
        ]
        if with_outline {
            outline.outline();
        }
    })
}
