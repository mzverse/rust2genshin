#let sequence = [].func();
#let space = [ ].func();
#let styled = {set page(fill: auto);}.func();
#let text_def = ("size": 12pt, "fill": black);
#let std_symbol = [~].func();

#let contentToString(con) = {
    if type(con) == str {
        con
    } else if con.has("text") {
        con.at("text")
    } else if con.has("body") {
        contentToString(con.at("body"))
    } else if con.has("children") {
        con.at("children").map(contentToString).join()
    } else {
        repr(con)
    }
}

#let html_elem(tag, attrs: (:), ..args) = context {
    if target() == "html" {
        return html.elem(tag, args.pos().join(), attrs: attrs + args.named());
    }
    if ("head", "script", "style", "template").contains(tag) {
        return none;
    }
    return args.pos().join();
}
#let html_inline-flex = html_elem.with("span", class: "inline-flex"); // TODO
#let html_frame(body) = context {
    if target() == "html" {
        html.frame(body)
    } else {
        body
    }
}

#let base_path = sys.inputs.at("base_path", default: "");
#let import_style(src) = {
    return html_elem("link", attrs: (href: base_path + src, rel: "stylesheet"))[];
}
#let import_script(src) = {
    return html_elem("script", attrs: (src: base_path + src))[]
}

#let hr = html_elem("hr")[];
