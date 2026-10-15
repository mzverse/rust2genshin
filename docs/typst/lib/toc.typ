#import "/lib/lib.typ": *;
#set raw(lang: "java");
#show: template.with(toc: false, with_outline: false);

#let hides = (
    "/lib",
    "/res",
)

#let gen(content, path: "") = {
    let result = ();
    for (name, value) in content.pairs() {
        if type(value) == dictionary {
            if hides.contains(path + name) {
                continue;
            };
            result.push[
                #if "index.html" in value {
                    link(path+name+"/index.html", value.at("index.html"))
                } else {
                    html.span(name)
                };
                #html_elem("button", attrs: (class: "open"))[]
                #gen(value, path: path+name+"/")
            ];
        } else {
            if name == "index.html" and path != "" {
                continue;
            };
            if hides.contains(path + name) {
                continue;
            };
            result.push(link(path+name, value));
        }
    };
    return list(..result);
}

#let to_tree(path, title) = {
    let pos = path.position("/");
    if pos == none {
        dictionary_item(path, title)
    } else {
        dictionary_item(path.slice(0, pos), to_tree(path.slice(pos + 1), title))
    }
}

#let merge(a, b) = {
    for (k, v) in b {
        let v0 = a.at(k, default: none);
        if v0 == none {
            a.insert(k, v);
            continue;
        }
        if type(v) == dictionary {
            a.at(k) = merge(v0, v);
        } else {
            panic(v);
        }
    }
    a
}

#context {
    let tree = query(document).sorted(key: x => current-order.at(x.location())).map(x => to_tree(x.path, x.title)).fold((:), merge);
    // 抽屉模式下的关闭按钮 —— 窄屏这一栏才是抽屉,那时它出现在右上角。
    // 显隐由 toc.css 的 media query 决定,点击由 toc.js 接。
    html_elem("button", attrs: (class: "drawer-close"))[✕] + gen(tree)
}
