#import "/lib/lib.typ": *

/// 把 Shoelace 引进来。**每个用到 `<sl-*>` 的地方都要调一次** ——
/// `custom_element` 的作用就是把回调挂到那一页的 `<head>` 上,没调就等于没引。
///
/// 按需注册组件,不用 `shoelace-autoloader.js`:autoloader 会挂一个
/// MutationObserver 盯着整页、按标签名懒加载,这一页只有按钮和下拉,多那一层
/// 没必要;而且它是 ES module,`import_script` 发的是普通 `<script>`,发不出
/// `type="module"`。
#let import_shoelace() = {
    // light.css 挂在 `:root`,dark.css 挂在 `.sl-theme-dark`。先 light 后 dark:
    // `<html class="sl-theme-dark">` 时两条规则特异性相同(都是 0,1,0),
    // 后加载的 dark 赢。切换在 lib/js/theme.js 里。
    import_style("https://cdn.jsdelivr.net/npm/@shoelace-style/shoelace@2.20.1/cdn/themes/light.css");
    import_style("https://cdn.jsdelivr.net/npm/@shoelace-style/shoelace@2.20.1/cdn/themes/dark.css");
    // 只覆盖主色 / 字体,不重画 Shoelace 的中性色阶。
    import_style("/lib/css/shoelace-theme.css");
    // `<i class="material-icons">` 只是把图标名交给 Material Icons 这个**字体**
    // 去渲染,字体没引的话位置上显示的是图标名那几个字母。
    import_style("https://fonts.googleapis.com/icon?family=Material+Icons");
    // `sl-option` 必须自己引:select.js 只带 popup / tag,**不注册 option**,
    // 没引的话 `<sl-option>` 是不 upgrade 的普通元素,没有 `.value`,父级
    // `<sl-select>` 同步不到选项,选中态永远是空。
    //
    // 顺序有意义(module script 按文档顺序执行):option 先定义,select
    // upgrade 时子元素已经在,才能一次性把选中态同步对。
    for path in (
        "components/button/button.js",
        "components/option/option.js",
        "components/select/select.js",
    ) {
        html_elem("script", attrs: (
            "type": "module",
            "src": "https://cdn.jsdelivr.net/npm/@shoelace-style/shoelace@2.20.1/cdn/" + path,
        ))[]
    }
}

/// `<sl-button>`。
///
/// `variant`:`default`(描边)/ `primary`(实心主色)/ `text`(无边框)。
/// `pill`:胶囊圆角,默认开 —— 工具条里的控件统一是胶囊形。
///
/// 无值属性(布尔属性)在 HTML 里写成 `attr=""`,不是省略值 —— 站点的
/// `html_elem` 包装要求属性值是字符串,给 `none` 会报
/// `expected string, found none`。
#let sl-button(
    body,
    id: none,
    variant: "default",
    icon: none,
    pill: true,
    disabled: false,
    hidden: false,
) = {
    custom_element(
        import_shoelace,
        html_elem(
            "sl-button",
            // `prefix` slot 是组件留的图标位。图标字体引在 `import_shoelace` 里。
            ..if icon != none {
                (html_elem("i", class: "material-icons", attrs: ("slot": "prefix"))[#icon],)
            } else {},
            body,
            id: id,
            variant: variant,
            ..if pill { ("pill": "") } else {},
            ..if disabled { ("disabled": "") } else {},
            ..if hidden { ("hidden": "") } else {},
        ),
    )
}

/// `<sl-option>`。只能放在 `<sl-select>`(或 `<sl-menu>`)里。
#let sl-option(value, body) = {
    custom_element(
        import_shoelace,
        html_elem("sl-option", value: value, body),
    )
}

/// `<sl-select>`。
///
/// `value` 不传时显示 `placeholder`。`hoist` 永远打开:下拉弹层挂到
/// `<body>`,不会被祖先的 `overflow` 裁掉。选项用 `sl-option` 拼。
///
/// 读值等组件 upgrade 之后才准,脚本里要先
/// `await customElements.whenDefined("sl-select")`。
#let sl-select(
    body,
    id: none,
    class: none,
    value: none,
    placeholder: none,
    aria_label: none,
    pill: true,
    disabled: false,
    hidden: false,
) = {
    custom_element(
        import_shoelace,
        html_elem(
            "sl-select",
            body,
            id: id,
            class: class,
            size: "medium",
            attrs: (
                "hoist": "",
                ..if pill { ("pill": "") } else {},
                ..if aria_label != none { ("aria-label": aria_label) } else {},
            ),
            ..if value != none { ("value": value) } else {},
            ..if placeholder != none { ("placeholder": placeholder) } else {},
            ..if disabled { ("disabled": "") } else {},
            ..if hidden { ("hidden": "") } else {},
        ),
    )
}
