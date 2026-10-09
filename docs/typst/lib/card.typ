#import "/lib/lib.typ": *;
#import "@preview/note-me:0.6.0";

#let card(kind, con, alt: none) = context {
    if target() == "html" {
        custom_element(() => {
            html_elem("template", id: "card")[
                #html_elem("div", part: "icon")
                #html_elem("div", part: "content")[
                    #html_elem("slot")[content]
                ]
            ]
            import_script("/lib/js/card.js");
            import_style("/lib/css/card.css");
        }, html.elem("ui-card", attrs: ("type": kind), con))
    } else if alt != none {
        alt(con)
    } else {
        [/ [#kind]: #con]
    }
}

#let card_info = card.with("info", alt: note-me.note);
#let card_tip = card.with("tip", alt: note-me.important);
#let card_attention = card.with("attention", alt: note-me.warning);
