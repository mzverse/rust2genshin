#import "/lib/lib.typ": *;

#let template(self) = {
    show heading: it => context {
        let list = query(heading.where(outlined: true).within(this-doc().location()).before(here()));
        let level = it.level;
        let id = contentToString(it.body);
        for x in list.rev() {
            if x.level >= level {
                continue;
            }
            level = x.level;
            id = contentToString(x.body) + "-" + id;
        }
        html_elem("section", it, attrs: (id: id))
    };
    self
}

// 这里只管**本页目录**(nav.outline)这一件事。顶栏是全站 chrome,
// 独立成 lib/header.typ,运行时由 <site-header> 注入 —— 见 arch/docs/custom-element。
#let outline() = custom_element(() => {
    import_style("/lib/css/outline.css");
    import_script("/lib/js/outline.js");
}, {
    context {
        let result = ();
        let path = ();
        for x in query(heading.where(outlined: true).within(this-doc().location())) + (heading(level: 1, "EOF"),) {
            while path.len() > 0 and path.last().level >= x.level {
                _ = path.pop();
            }
            path.push(x);
            while result.len() > 1 and result.first().at(0) < result.last().at(0) and result.last().at(0) >= x.level {
                let j = 0;
                while result.at(j).at(0) < result.last().at(0) {
                    j += 1;
                }
                result.at(j - 1) += result.slice(j);
                result = result.slice(0, j);
            }
            result.push((x.level, link("#" + path.map(x => contentToString(x.body)).join("-"), x.body)));
        }
        result.pop();
        let f(a) = {
            return list(..a.map(c => if type(c) == content { c } else [
                #c.at(1);
                #f(c.slice(2));
            ]));
        };
        return html_elem("nav", attrs: (class: "outline"),
            html_elem("button", attrs: (class: "outline-close"))[✕] + f(result));
    }
})

// test
#context {
    let h = here();
    [
        = h1
        == h2
        === h3
        = h11
        === h33
        ==== h4
        == h22
        #outline();
    ]
}
