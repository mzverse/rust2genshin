#import "/lib/lib.typ": *;
// base_path 定义在 contents.typ。template.typ 的 `import ...: *` 不会把
// 它的 import 再导出一遍,所以这里要单独引一次。
#import "contents.typ": *;

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

#let outline() = custom_element(() => {
    import_style("/lib/css/outline.css");
    import_script("/lib/js/outline.js");
}, {
    // 顶栏。它是 body 的直接子元素(template.typ 在 main 之外调 outline()),
    // 所以不落在 Typst 插的 .block/.container 包装里,grid 行位置可预测。
    // sticky —— ☰ 原来是 position: fixed,滚动时始终够得着;挪进普通顶栏
    // 会让它们滚没。
    // 左栏按钮也跟着放这儿 —— 本站每页要么 `toc` 和 `with_outline` 一起开、
    // 要么一起关(toc.typ),所以抽屉的样式和脚本在 outline.css / outline.js
    // 里一份就够。
    // 三个按钮都带 aria-label:☰ 本身是图形,不给名字读屏只会念「按钮」。
    // theme-toggle 那条是给 JS 前的一版静态文案,theme.js 在
    // DOMContentLoaded 时会按**目标**状态(而不是当前状态)改写它。
    html_elem("header", attrs: (class: "site-header"), {
        html_elem("div", attrs: (class: "header-left"), {
            html_elem("button", attrs: (class: "toc-toggle", aria-label: "全站目录"))[☰]
            html_elem("a", attrs: (class: "site-title", href: base_path + "/index.html"))[rust2genshin]
        })
        html_elem("div", attrs: (class: "header-right"), {
            html_elem("button", attrs: (class: "theme-toggle", aria-label: "切换主题"))[]
            html_elem("button", attrs: (class: "outline-toggle", aria-label: "本页目录"))[☰]
        })
    })
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
