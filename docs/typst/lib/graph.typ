#import "/lib/lib.typ": *;

/*
 * 简单的节点图,写法照着 HTML 的样子来:元素套元素,顺序就是顺序。
 *
 *   #import "/lib/graph.typ" as graph
 *
 *   #{
 *     import graph: *
 *     graph.graph(
 *       node("entry", {
 *         [入口]
 *         ctrl_out()
 *         data_out("Entity")[玩家]
 *       })
 *       node("set", {
 *         [SetLocal\<Int\>]
 *         ctrl_in()
 *         data_in("Int")[值]
 *       })
 *       ctrl(("entry", 0), ("set", 0))
 *     )
 *   }
 *
 * `graph` 收位置参数。`{ ... }` 里的多个语句会被自动 join 成一个内容序列,
 * 所以「一块内容」和「逐个传参」等价,`graph(body)` 和 `graph(a, b, c)` 都收。
 *
 * 引脚没有 id —— 同一节点里同 `kind` + 同 `dir` 的引脚算一组,连接里写的就是
 * 组内序号(从 0 起)。`in`/`out` × `ctrl`/`data` 四组各数各的。
 *
 * `node` 也可以不放进 `graph()`,单独摆一个看引脚长什么样。
 */

/* ------------------------------------------------------------------ *
 * 元素
 * ------------------------------------------------------------------ */

/// 注册用的外壳。`ui-graph` 和 `ui-graph-node` 共用同一份注册:内容一字不差,
/// `template.typ` 那边的 `.dedup()` 会合成一个,同一个页面里图和单独的节点
/// 混着用也只会引一次脚本。
#let graph_element(ele) = custom_element(() => {
  import_script("/lib/js/graph.js");
  import_style("/lib/css/graph.css");
}, ele)

/// 一根引脚。位置参数:`dir`、`kind`、`type`,后面挂可选的 `[label]`。
///
/// 平时用 `ctrl_in` / `ctrl_out` / `data_in` / `data_out` 四个短的,`pin` 只是它们的
/// 共同底座。
///
/// - `dir`:`"in"` / `"out"`
/// - `kind`:`"ctrl"` / `"data"`
/// - `type`:数据类型,只在悬停时浮出来;控制流没有类型,写 `""`
/// - `[label]`:显示在引脚旁边的名字,不写就没有
///
/// `label` 用 `..label` 收而不是给默认值:Typst 不允许用位置参数填有默认值的
/// 形参,那样写会直接报 unexpected argument。
#let pin(dir, kind, type, ..label) = {
  assert(
    dir == "in" or dir == "out",
    message: "graph: 引脚 dir 只能是 in / out,得到 " + repr(dir),
  )
  assert(
    kind == "ctrl" or kind == "data",
    message: "graph: 引脚 kind 只能是 ctrl / data,得到 " + repr(kind),
  )
  let pos = label.pos()
  assert(
    pos.len() <= 1,
    message: "graph: pin 的位置参数最多四个:dir, kind, type, label",
  )
  html_elem("pin", attrs: (
    data-dir: dir,
    data-kind: kind,
    data-type: contentToString(type),
  ), contentToString(pos.at(0, default: "")))
}

/// 控制流入口引脚。控制流没有类型,也没有标签。
#let ctrl_in(..name) = pin("in", "ctrl", "", ..name);

/// 控制流出口引脚。
#let ctrl_out(..name) = pin("out", "ctrl", "", ..name);

/// 数据流入脚。位置参数:`type`,后面挂可选的 `[label]`。
#let data_in(type, ..label) = pin("in", "data", type, ..label);

/// 数据流出脚。位置参数:`type`,后面挂可选的 `[label]`。
#let data_out(type, ..label) = pin("out", "data", type, ..label);

/// 一个节点。位置参数:`id`、内容块,或者只有内容块。
///
/// 内容块里标题写在最前面,后面跟引脚。`id` 省略时(只是单独摆一个看看形状,
/// 不需要被连接引用)不必费劲起名。
///
/// ```typ
/// node("set", { [SetLocal\<Int\>] ctrl_in() })
/// node({ [入口] ctrl_out() })
/// ```
///
/// 放进 `graph()` 里就是图中的一个节点;单独摆(不放进图)也能用 —— 注册
/// 脚本的那份说明靠 `template.typ` 的 `dedup()` 合掉,重复写不花钱。导出 pdf
/// 之类非 html 的目标下只留下内容文本。
#let node(..args) = context {
  let pos = args.pos()
  // 两个位置参数时头一个是 id,一个时是内容 —— 内容总是块,id 总是字符串。
  let (id, body) = if pos.len() >= 2 and type(pos.at(0)) == str {
    (pos.at(0), pos.slice(1).join())
  } else {
    (none, pos.join())
  }
  if target() != "html" {
    return contentToString(body)
  }
  let attrs = if id == none { (:) } else { (id: id) }
  // body 必须用方括号包成 trailing block —— 光写个裸名字会被 Typst 当成
  // 具名参数 body: 吞进属性里,元素反而是空的。
  graph_element(html_elem("ui-graph-node", attrs: attrs)[#body])
}

/// 一条连接。位置参数:`kind`、起点 `(节点 id, 组内序号)`、终点同样。
///
/// 平时用 `ctrl` / `data` 两个短的,`connection` 只是它们的共同底座。
///
/// 序号的两端各数各的:起点按 `out` 数,终点按 `in` 数。
///
#let connection(kind, from, to) = {
  assert(
    kind == "ctrl" or kind == "data",
    message: "graph: 连接的 kind 只能是 ctrl / data,得到 " + repr(kind),
  )
  for (name, end) in (("起点", from), ("终点", to)) {
    assert(
      type(end) == array and end.len() == 2,
      message: "graph: 连接" + name + "应当写成 (节点 id, 组内序号),得到 " + repr(end),
    )
    assert(
      type(end.at(0)) == str,
      message: "graph: 连接" + name + "的节点 id 应当是字符串,得到 " + repr(end.at(0)),
    )
    assert(
      type(end.at(1)) == int,
      message: "graph: 连接" + name + "的引脚应当写组内序号(整数),得到 " + repr(end.at(1)),
    )
  }
  html_elem("connection", attrs: (
    data-kind: kind,
    data-from-node: from.at(0),
    data-from-index: str(from.at(1)),
    data-to-node: to.at(0),
    data-to-index: str(to.at(1)),
  ))
}

/// 一条控制流连接。位置参数:起点 `(节点 id, 组内序号)`、终点同样。
#let ctrl(from, to) = connection("ctrl", from, to)

/// 一条数据流连接。位置参数:起点 `(节点 id, 组内序号)`、终点同样。
#let data(from, to) = connection("data", from, to)

/* ------------------------------------------------------------------ *
 * 图
 * ------------------------------------------------------------------ */

/// 一张简单的节点图。收位置参数,元素按写的顺序排。
///
/// - 左键拖节点,右键 / 中键拖整张图(左键在空白处不动),滚轮缩放
/// - 生成时做一次分层自动整理(确定性的,同一份文档每次位置一样)
/// - 悬停节点高亮它的进出线
///
/// - `height`: 图的高度,默认 `260pt`
/// - `alt`: 目标不是 html(比如导出 pdf)时的替身
///
/// ```typ
/// #graph(
///   node("entry", {
///     [入口]
///     ctrl_out()
///     data_out("Entity")[玩家]
///   }),
///   node("set", {
///     [SetLocal\<Int\>]
///     ctrl_in()
///     data_in("Int")[值]
///   }),
///   ctrl(("entry", 0), ("set", 0)),
///   data(("entry", 0), ("set", 0)),
/// )
/// ```
#let graph(..args, height: 260pt, alt: none) = context {
  let items = args.pos()
  if target() != "html" {
    return if alt != none {
      alt
    } else {
      [节点图:#items.len() 个元素]
    }
  }
  // 元素按写的顺序铺进 <ui-graph>,谁来读、怎么连线交给 JS —— 它就按
  // HTML 结构找 <ui-graph-node> 和 <connection>。这里不预先分组。
  let body = (html_elem("div", attrs: (class: "graph-fallback"))[
      需要 JavaScript 才能画出来的节点图
    ],)
  body += items
  graph_element(html.elem("ui-graph", attrs: ("data-height": repr(height)), body.join()))
}
