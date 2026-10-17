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
    #html_elem("input", id: "play-sample", attrs: (list: "play-samples"), placeholder: "样例")
    #html_elem("button", id: "play-sample-load", type: "button")[载入]
    // 样例是**静态资产**(`play/demo/*.rs` 会被当 asset 拷进输出目录),
    // 不从后端取 —— 页面挂了样例也还在,不用等后端起来。
    // 下拉框里只显示文件名(去掉目录和 `.rs`),真路径塞进 `label`:
    // 浏览器渲染 datalist 时 `value` 是主文本、`label` 是次要文本,路径不会
    // 顶在前面刷屏;script.js 从 `label` 取回真路径(见 `SAMPLES` 那张表)。
    #context html_elem("datalist", attrs: (id: "play-samples"))[
      #for x in query(asset).map(x => x.path).filter(x => x.starts-with("/play/demo/")) {
        html.option(value: x.split("/").last().trim(".rs", at: end), label: base_path + x);
      }
    ]
  ]
  #html_elem("div", class: "play-layout")[
    #html_elem("textarea", class: "play-editor", id: "play-editor", spellcheck: "false", autocomplete: "off")
    #html_elem("div", class: "play-right")[
      #html_elem("div", class: "play-head")[
        #html_elem("span", id: "play-status", class: "play-status")[]
        #html_elem("span", class: "play-actions")[
          #html_elem("button", id: "play-run", type: "button")[编译]
          #html_elem("button", id: "play-cancel", type: "button", hidden: "")[取消]
          #html_elem("button", id: "play-download", type: "button", hidden: "")[下载 .gia]
        ]
      ]
      #html_elem("pre", id: "play-log", class: "play-log")
    ]
  ]
]