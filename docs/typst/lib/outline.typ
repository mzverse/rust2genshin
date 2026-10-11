#import "/lib/lib.typ": *;

#let template(self) = {
    show heading: it => context {
        let list = query(selector(heading).within(this-doc.get()).before(here()));
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

#let outline() = custom_element(() => {
    import_style("/lib/css/outline.css");
    import_script("/lib/js/outline.js");
}, {
    // 两个抽屉的开关:左上开全站目录(toc-component),右上开本页目录。
    // 左栏按钮也跟着放这儿 —— 本站每页要么 `toc` 和 `with_outline` 一起开、
    // 要么一起关(toc.typ),所以抽屉的样式和脚本在 outline.css / outline.js
    // 里一份就够。
    html_elem("button", attrs: (class: "toc-toggle"))[☰]
    html_elem("button", attrs: (class: "outline-toggle"))[☰]
    context {
        let result = ();
        let path = ();
        for x in query(selector(heading).within(this-doc.get())) + (heading(level: 1, "EOF"),) {
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
    this-doc.update(x => h);
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
