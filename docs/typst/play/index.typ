#import "/lib/lib.typ": *;
// base_path 定义在 contents.typ,lib.typ 的 `import …: *` 不会把它再导出来。
#import "/lib/contents.typ": *;
// 顶栏左侧那两个选项卡的另一半。toc / with_outline 都关掉:这一页没有
// 目录可列,`#let title` 已经由 template 渲染成 <h1> 了。
#show: template.with(toc: false, with_outline: false);

#let title = [摆弄];
#let order = 2;

#set raw(lang: "rust");

#custom_element(() => {
    import_style("/play/style.css");
    import_script("/play/script.js");
})[]

左半边写 Rust,右半边是编译日志。点「编译」把源码发到后端,后端在一个
一次性容器里跑 `cargo build`,回来的 `.gia` 可以直接下载进游戏。

第一次编译可能要多等几分钟 —— 后端要先烘一个镜像出来,`/healthz`
在那之前会报 `not_ready`。有多个后端,页面会挨个试,连上哪个用哪个。

// 页面里**不出现**后端地址,也不给改:地址表写死在 script.js 的
// `BACKENDS` 里,页面唯一给后端的是 `data-*` 的静态资产路径约定。
#html_elem("div", class: "play")[
  #html_elem("div", class: "play-toolbar")[
    #html_elem("label", class: "play-label")[示例]
    // 用 `<select>` 而不是 `<input list>` + datalist:datalist 是个**随键入
    // 过滤**的输入框,选过一次之后列表里就只剩匹配的那一项,另外三个样例得
    // 先清空输入才看得见;`<select>` 永远把全部选项列出来。
    //
    // `value` 放真路径、显示名(去掉目录和 `.rs`)做标签正文 —— script.js 直接
    // 拿 `value` 去 fetch,不用再维护一张名字到路径的映射表。
    // 名字写在正文里而不是 `label` 属性上:`<option label>` 的显示文本各浏览器
    // 行为不统一(Chrome 有内容时以内容为准),正文是确定的。
    #context html_elem("select", id: "play-sample")[
      #html.option(value: "", selected: true)[选一个…]
      #for x in query(asset).map(x => x.path).filter(x => x.starts-with("/play/demo/")) {
        html.option(value: base_path + x)[#x.split("/").last().trim(".rs", at: end)]
      }
    ]
    #html_elem("button", id: "play-sample-load", type: "button")[载入]
    #html_elem("button", id: "play-run", type: "button")[编译]
  ]
  #html_elem("div", class: "play-layout")[
    #html_elem("textarea", class: "play-editor", id: "play-editor", spellcheck: "false", autocomplete: "off")
    #html_elem("div", class: "play-right")[
      #html_elem("div", class: "play-head")[
        #html_elem("span", id: "play-status", class: "play-status")[]
        #html_elem("span", class: "play-actions")[
          #html_elem("button", id: "play-cancel", type: "button", hidden: "")[取消]
          #html_elem("button", id: "play-download", type: "button", hidden: "")[下载 .gia]
        ]
      ]
      #html_elem("pre", id: "play-log", class: "play-log")
    ]
  ]
]