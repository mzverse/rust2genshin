#import "contents.typ": *;

#let this-path = state("this-path");

#let template(con, toc: true, with_outline: true, title: none) = context {
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
        show: style.templateStyle;
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
            import_script("/lib/js/style.js");
            import_style("/lib/css/style.css");
            import_style("/lib/css/template.css");
            include math_render;
            if toc {
                import_script("/lib/js/toc.js");
            }
            for x in query(<custom-element>).dedup() {
                (x.value)();
            }
        });
        if toc {
            html_elem("toc-component")[];
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
