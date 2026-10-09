#import "./meta.typ";

#import "contents.typ";
#import contents: *;

#let template(con, toc: true, with_outline: true, title: none) = {
    set text(lang: "zh", font: "Microsoft YaHei", size: 12pt);
    show raw: set text(font: ("Consolas", "Microsoft YaHei"));
    html_elem("meta", attrs: (name: "root", content: meta.root));
    include contents;
    import_style("/lib/css/template.css");
    import "style.typ";
    show: style.templateStyle;
    include style;
    import "math_render.typ";
    show: math_render.template;
    include math_render;
    import "code_block.typ";
    show: code_block.template;
    context if document.title != none {
        html_elem("title", contentToString(document.title));
    }
    if toc {
        import_script("/lib/js/toc.js");
        html_elem("toc-component")[];
    }
    import "outline.typ" as outline;
    let page_style = if with_outline { outline.template } else { x => x };
    context {
        for x in query(<custom-element>).dedup() {
            (x.value)();
        }
    }
    context if target() == "html" {
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
    } else {
        con
    }
}
