#import "/lib/lib.typ": *;
// base_path 定义在 contents.typ,lib.typ 的 `import …: *` 不会把它再导出来。
#import "/lib/contents.typ": *;
// 顶栏左侧那两个选项卡的另一半。toc / with_outline 都关掉:这一页没有
// 目录可列,`#let title` 已经由 template 渲染成 <h1> 了。
#show: template.with(toc: false, with_outline: false);

#let title = [摆弄];
#let order = 2;

#set raw(lang: "rust");

#import_style("/play/style.css");
#import_script("/play/script.js");

左半边写 Rust,右半边是编译日志。点「编译」把源码发到后端,后端在一个
一次性容器里跑 `cargo build`,回来的 `.gia` 可以直接下载进游戏。

第一次编译可能要多等几分钟 —— 后端要先烘一个镜像出来,`/healthz`
在那之前会报 `not_ready`。

// `data-backend` 是「这个页面连哪个后端」的唯一出处;script.js 拿它当初始
// 值(用户在输入框里改过的话,以 localStorage 为准)。
#html_elem("div", class: "play", attrs: (data-backend: "wss://www.mzverse.org:6/rust2genshin/ws"))[
  #html_elem("div", class: "play-toolbar")[
    #html_elem("input", id: "play-sample", attrs: (list: "play-samples"), placeholder: "选个样例")
    #html_elem("button", id: "play-sample-load", type: "button")[载入]
    #html_elem("button", id: "play-run", type: "button")[编译]
    #html_elem("button", id: "play-cancel", type: "button", hidden: "")[取消]
    #html_elem("button", id: "play-download", type: "button", hidden: "")[下载 .gia]
    // 样例是**静态资产**(`play/demo/*.rs` 会被当 asset 拷进输出目录),
    // 不从后端取 —— 页面挂了样例也还在,不用等后端起来。
    #context html_elem("datalist", attrs: (id: "play-samples"))[
      #for x in query(asset).map(x => x.path).filter(x => x.starts-with("/play/demo/")) {
        html.option(value: base_path + x);
      }
    ]
  ]
  #html_elem("div", class: "play-layout")[
    #html_elem("textarea", class: "play-editor", id: "play-editor", spellcheck: "false", autocomplete: "off")
    #html_elem("div", class: "play-right")[
      #html_elem("div", id: "play-status", class: "play-status")[]
      #html_elem("pre", id: "play-log", class: "play-log")
    ]
  ]
  #html_elem("div", class: "play-backend")[
    #html_elem("label")[
      后端地址
      #html_elem("input", id: "play-backend", type: "text")
    ]
    #html_elem("span", class: "play-hint")[改这个要重新加载页面;地址存在 localStorage 里]
  ]
]