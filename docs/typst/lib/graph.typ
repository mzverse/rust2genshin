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

/// 注册本体。单独抽出来是因为 `.dedup()` 按**注册函数**判等:所有用到
/// `ui-graph` 的页面必须传同一个函数值,否则同样的标签会被发两遍,
/// 第二个 `graph.js` 会在 `const SVG_NS` 上重复声明。
#let graph_register_impl = () => {
  import_script("/lib/js/graph.js");
  import_style("/lib/css/graph.css");
}

/// 只要注册、不要元素(`play/index.typ` 用)。注册内容与 `graph_element` 相同,
/// 且共享同一个函数值 —— 两者出现在同一页也只会引一次脚本。
#let graph_register() = custom_element(graph_register_impl, [])

/// 注册用的外壳。`ui-graph` 和 `ui-graph-node` 共用同一份注册:内容一字不差,
/// `template.typ` 那边的 `.dedup()` 会合成一个,同一个页面里图和单独的节点
/// 混着用也只会引一次脚本。
#let graph_element(ele) = custom_element(graph_register_impl, ele)

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
  ), pos.at(0, default: ""))
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
/// 可选 `comment`:节点上方的一个小注框,和 `.gia` 预览里那个
/// `attached_comment` 长得一样。讲「这个变量叫什么」这类图上写不下的信息时用。
///
/// ```typ
/// node("local", comment: "变量 phase", {
///   [获取局部变量]
///   data_out("LocalRef")[变量引用]
/// })
/// ```
///
/// 放进 `graph()` 里就是图中的一个节点;单独摆(不放进图)也能用 —— 注册
/// 脚本的那份说明靠 `template.typ` 的 `dedup()` 合掉,重复写不花钱。导出 pdf
/// 之类非 html 的目标下只留下内容文本。
#let node(..args, comment: none) = context {
  let pos = args.pos()
  // 两个位置参数时头一个是 id,一个时是内容 —— 内容总是块,id 总是字符串。
  let (id, body) = if pos.len() >= 2 and type(pos.at(0)) == str {
    (pos.at(0), pos.slice(1).join())
  } else {
    (none, pos.join())
  }
  if target() != "html" {
    let text = contentToString(body)
    if comment != none {
      text = contentToString(comment) + " / " + text
    }
    return text
  }
  let attrs = if id == none { (:) } else { (id: id) }
  // 注释必须是节点的**直系子元素** —— graph.js 只在这一层找它
  // (`[...this.children].find(...)`),挂在别处不会被认领。
  //
  // 顺序也有讲究:GraphNode.build 先扫注释、再 #readPins,而注释框的尺寸
  // 由它自己的 shadow 决定,所以要排在正常内容之前建好。
  let children = if comment == none {
    body
  } else {
    html_elem("ui-graph-comment", comment) + body
  }
  // body 必须用方括号包成 trailing block —— 光写个裸名字会被 Typst 当成
  // 具名参数 body: 吞进属性里,元素反而是空的。
  graph_element(html_elem("ui-graph-node", attrs: attrs)[#children])
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

/// 引脚旁的一小段附加信息。两种用处,各占一组函数:
///
/// - *默认值* —— 只有数据入脚有值可写:`data_default(at, text)`
/// - *外部接线* —— 这根引脚连到图外面。四种引脚各一个:
///   `data_in_extern` / `data_out_extern` /
///   `ctrl_in_extern` / `ctrl_out_extern`
///
/// 全部只收 `at` = `(节点 id, 组内序号)`。序号在**那一组**里数:节点上
/// `data-in` / `data-out` / `ctrl-in` / `ctrl-out` 四组各数各的。
///
/// ```typ
/// node("log1", {
///   [打印日志]
///   ctrl_in()
///   data_in("String")[内容]
/// }),
/// data_default(("log1", 0))[`"第一阶段"`]   // 内容引脚上写个常量
/// ctrl_in_extern(("log1", 0))             // 控制流从图外来
/// ```
///
/// 它们都只是数据载体:元素是 `<ui-graph>` 的直接子节点,位置由 `graph.js`
/// 算出来摆在节点**外侧**(入口在左边界外、出口在右边界外),不和盒子里那侧
/// 的引脚名抢地方。
///
/// *写字符串值时要注意智能引号。* Typst 会把内容里的 `"..."` 变成弯引号
/// `“...”`,直接用 `["第一阶段"]` 出来的是弯的。节点图里的字符串常量是带
/// 直引号的,所以套一层行内代码把引号原样留住:
///
/// ```typ
/// data_default(("log1", 0))[`"第一阶段"`]
/// ```
///
/// 数值不用这样 —— `[100]` 就是对的。
///
/// 底座:下面五个都是它的封装,平时不用直接调。
#let pin_info(pin_kind, dir, at, text, kind: none) = {
  assert(
    pin_kind == "data" or pin_kind == "ctrl",
    message: "graph: pin_info 的引脚种类只能是 data / ctrl,得到 " + repr(pin_kind),
  )
  assert(
    dir == "in" or dir == "out",
    message: "graph: pin_info 的 dir 只能是 in / out,得到 " + repr(dir),
  )
  assert(
    kind == none or kind == "extern",
    message: "graph: pin_info 的 kind 只能是 none / extern,得到 " + repr(kind),
  )
  assert(
    type(at) == array and at.len() == 2,
    message: "graph: pin_info 应当写成 (节点 id, 组内序号),得到 " + repr(at),
  )
  assert(
    type(at.at(0)) == str,
    message: "graph: pin_info 的节点 id 应当是字符串,得到 " + repr(at.at(0)),
  )
  assert(
    type(at.at(1)) == int,
    message: "graph: pin_info 的引脚应当写组内序号(整数),得到 " + repr(at.at(1)),
  )
  // kind 不写就整个属性都不发 —— 渲染层用 `|| 'default'` 兜底(和 node 的
  // comment 一个路子:不写就什么都不加)。
  let attrs = (
    data-node: at.at(0),
    data-dir: dir,
    data-kind: pin_kind,
    data-index: str(at.at(1)),
  )
  if kind != none {
    attrs.insert("kind", kind)
  }
  html_elem("pin-info", attrs: attrs, text)
}

/// 数据入脚上的*默认值*。引脚上没有连线时生效的那个值。
///
/// ```typ
/// data_default(("log1", 0))[100]
/// ```
#let data_default(at, text) = pin_info("data", "in", at, text)

/// 以下四个表示这根引脚连到图外。命名按 `<种类>_<方向>` 拼:
/// `data` / `ctrl` × `in` / `out`。

/// 数据入脚连到图外 —— 参数从外面进来。
#let data_in_extern(at) = pin_info("data", "in", at, "", kind: "extern")

/// 数据出脚连到图外 —— 返回值交给外面。
#let data_out_extern(at) = pin_info("data", "out", at, "", kind: "extern")

/// 控制流入脚连到图外 —— 流程从图外开始。
#let ctrl_in_extern(at) = pin_info("ctrl", "in", at, "", kind: "extern")

/// 控制流出脚连到图外 —— 流程到这里结束。
#let ctrl_out_extern(at) = pin_info("ctrl", "out", at, "", kind: "extern")

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
