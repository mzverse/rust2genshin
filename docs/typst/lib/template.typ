#import "contents.typ": *;

#let this-doc = state("this-doc");

#let template(con, toc: true, with_outline: true, title: none) = context {
    set text(lang: "zh", font: "Microsoft YaHei", size: 12pt);
    show link: if target() == "html" { it => {
        let dest = it.dest;
        if type(dest) == str and dest.starts-with("/") {
            html.a(href: base_path + dest, it.body)
        } else {
            it
        }
    } } else { it => it };
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
            html.base(href: base_path + this-doc.get());
            if document.title != none {
                html_elem("title", contentToString(document.title));
            }
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
            #con
        ]
        if with_outline {
            include outline;
        }
    })
}
