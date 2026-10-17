#import "/lib/lib.typ": *;
// base_path 定义在 contents.typ,lib.typ 的 `import …: *` 不会把它再导出来。
#import "/lib/contents.typ": *;
// 顶栏左侧那两个选项卡的另一半。toc / with_outline 都关掉:这一页没有
// 目录可列,`#let title` 已经由 template 渲染成 <h1> 了。
#show: template.with(toc: false, with_outline: false);
// MDUI 的封装都在这儿。
#import "/lib/mdui.typ": mdui-select, mdui-button, mdui-text-field, mdui-icon

#let title = [摆弄];
#let order = 2;

#set raw(lang: "rust");

#import_style("/play/style.css");
#import_script("/play/script.js")

左半边写 Rust,右半边是编译日志。点「编译」把源码发到后端,后端在一个
一次性容器里跑 `cargo build`,回来的 `.gia` 可以直接下载进游戏。

第一次编译可能要多等几分钟 —— 后端要先烘一个镜像出来,`/healthz`
在那之前会报 `not_ready`。有多个后端,页面会挨个试,连上哪个用哪个。

// 页面里**不出现**后端地址,也不给改:地址表写死在 script.js 的
// `BACKENDS` 里。
#html_elem("div", class: "play")[
  #html_elem("div", class: "play-toolbar")[
    // 「示例 + 选 + 载入」框成一组:它是「往编辑器里放段现成代码」,和「编译」
    // 是两件事,混在一排里看不出来。
    #html_elem("div", class: "play-picker")[
      #html_elem("span", class: "play-label")[示例：]
      // 样例是**静态资产**(`play/demo/*.rs` 会被当 asset 拷进输出目录),
      // 不从后端取 —— 页面挂了样例也还在,不用等后端起来。
      // `value` 放真路径、显示名(去掉目录和 `.rs`)做标签正文,script.js
      // 直接拿 `.value` 去 fetch。
      #context {
        // 键是真路径、值是显示名。`("a/b": …)` 这种带斜杠的键必须加引号。
        let options = (:)
        for x in query(asset).map(x => x.path).filter(x => x.starts-with("/play/demo/")) {
          options.insert(base_path + x, [#x.split("/").last().trim(".rs", at: end)])
        }
        // `<select>` 会自动选中第一个,`<mdui-select>` 不会 —— 它得显式给
        // `value`。不给的话框里是空的,页面一进来就没法直接「载入」。
        mdui-select(options, value: options.keys().at(0), id: "play-sample", variant: "outlined")
      }
      #mdui-button(id: "play-sample-load", variant: "outlined", icon: "download")[载入]
    ]
    #mdui-button(id: "play-run", variant: "filled", icon: "play_arrow")[编译]
  ]
  #html_elem("div", class: "play-layout")[
    #mdui-text-field(id: "play-editor", variant: "outlined", textarea: true, rows: "20")
    #html_elem("div", class: "play-right")[
      #html_elem("div", class: "play-head")[
        #html_elem("span", id: "play-status", class: "play-status")[]
        #html_elem("span", class: "play-actions")[
          #mdui-button(id: "play-cancel", variant: "outlined", icon: "close", hidden: true)[取消]
          #mdui-button(id: "play-download", variant: "outlined", icon: "save_alt", hidden: true)[下载 .gia]
        ]
      ]
      #html_elem("pre", id: "play-log", class: "play-log")
    ]
  ]
]
