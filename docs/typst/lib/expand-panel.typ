#import "/lib/lib.typ": *;

#let expand-panel(trigger, con, open: false) = context {
    if target() == "html" {
        custom_element(() => {
            html_elem("template", id: "expand-panel")[
                #html_elem("div", part: "header")[
                    #html_elem("slot", name: "trigger")[trigger]
                    #html_elem("div", part: "arrow")[▼]
                ]
                #html_elem("div", part: "content")[
                    #html_elem("div", part: "inner")[
                        #html_elem("slot", name: "content")[content]
                    ]
                ]
            ]
            import_script("/lib/js/expand-panel.js");
            import_style("/lib/css/expand-panel.css");
        }, html.elem("expand-panel", html.div(trigger, slot: "trigger") + html.div(con, slot: "content"), attrs: if open {("open":"")} else {(:)}))
    } else {
        [/ #trigger: \ #con]
    }
}
