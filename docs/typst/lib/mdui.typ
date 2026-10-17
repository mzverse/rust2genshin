#import "/lib/lib.typ": *

/// 把 MDUI 的 CSS/JS 引进来。**每个用到 MDUI 组件的地方都要调一次** ——
/// `custom_element` 的作用就是把回调挂到那一页上,没调就等于没引。
#let import_mdui() = {
    import_style("https://unpkg.com/mdui@2/mdui.css");
    // `<mdui-icon name="…">` 只是把 name 交给 Material Icons 这个**字体**去
    // 渲染(`mdui-icon` 的 `font-family` 就是它),字体没引的话图标位置上显示的
    // 就是图标名那几个字母。MDUI 自己不带这个字体,得单独引一份。
    import_style("https://fonts.googleapis.com/icon?family=Material+Icons");
    import_script("https://unpkg.com/mdui@2/mdui.global.js");
}

/// `<mdui-select>`。`options` 是「值 -> 显示文字」的字典。
///
/// 无值属性(布尔属性)在 HTML 里写成 `attr=""`,不是省略值 —— 站点的
/// `html_elem` 包装要求属性值是字符串,给 `none` 会报
/// `expected string, found none`。
///
/// 选中状态要等组件 upgrade 之后才同步到 JS 的 `.value` 上,所以在 script
/// 里读它得先 `await customElements.whenDefined("mdui-select")`。
#let mdui-select(
    options,
    value: none,
    multiple: false,
    placeholder: none,
    label: none,
    variant: "outlined",
    id: none,
) = {
    custom_element(
        import_mdui,
        html_elem(
            "mdui-select",
            // 字典迭代直接给 (键, 值),不用解构数组 —— 传数组进来 `.at(0)`
            // 拿到的是下标而不是路径("expected string, found integer")。
            ..for (k, v) in options { (html_elem("mdui-menu-item", value: k, v),) },
            id: id,
            variant: variant,
            ..if value != none { ("value": value) } else {},
            ..if multiple { ("multiple": "") } else {},
            ..if placeholder != none { ("placeholder": placeholder) } else {},
            ..if label != none { ("label": label) } else {},
        ),
    )
}

/// `<mdui-button>`。`variant` 见文档:`elevated` / `filled` / `tonal` /
/// `outlined` / `text`。
#let mdui-button(body, id: none, variant: "text", icon: none, disabled: false, hidden: false) = {
    custom_element(
        import_mdui,
        html_elem(
            "mdui-button",
            ..if icon != none { (html_elem("mdui-icon", attrs: (slot: "icon", name: icon)),) } else {},
            body,
            id: id,
            variant: variant,
            ..if disabled { ("disabled": "") } else {},
            // `hidden` 同样得写 `""` 而不是省略 —— 见上面 `mdui-select` 的说明。
            ..if hidden { ("hidden": "") } else {},
        ),
    )
}

/// `<mdui-text-field>`。单行;要多行给 `textarea: true` 和 `rows`。
#let mdui-text-field(
    id: none,
    value: "",
    label: none,
    placeholder: none,
    variant: "outlined",
    icon: none,
    rows: none,
    textarea: false,
) = {
    custom_element(
        import_mdui,
        html_elem(
            "mdui-text-field",
            id: id,
            variant: variant,
            value: value,
            rows: rows,
            ..if textarea { ("textarea": "") } else {},
            ..if label != none { ("label": label) } else {},
            ..if placeholder != none { ("placeholder": placeholder) } else {},
            ..if icon != none { ("icon": icon) } else {},
        ),
    )
}

/// `<mdui-icon>`,Material Symbols 的一个图标。`name` 是图标名(如 `download`)。
#let mdui-icon(name) = {
    custom_element(import_mdui, html_elem("mdui-icon", attrs: (name: name)))
}
