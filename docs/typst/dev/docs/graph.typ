#import "/lib/lib.typ": *;
#let title = [节点图];
#show: template;

#import "/lib/graph.typ";

= 导入依赖

```typ
#import "/lib/graph.typ";
```

= 插入节点图

左键拖节点,右键或中键拖整张图(左键在空白处不动),滚轮缩放,悬停节点会高亮它的进出线。
把指针移到数据流圆点或连线上会浮出类型。

写法照着 HTML 的样子来:元素套元素,顺序就是顺序。下面先看效果,最后给源码。

#graph.graph({
    import graph: *;
    node("entry", {
        [入口]
        ctrl_out()
        data_out("Entity")[玩家]
    })
    node("local", {
        [Local\<Int\>]
        data_in("Int")[值]
        data_out("Int")[值]
    })
    node("add", {
        [Addition]
        data_in("Int")[左]
        data_in("Int")[右]
        data_out("Int")[和]
    })
    node("set", {
        [SetLocal\<Int\>]
        ctrl_in()
        data_in("Int")[值]
        ctrl_out()
    })
    node("log", {
        [Print String]
        ctrl_in()
        data_in("Str")[文本]
        ctrl_out()
    })
    ctrl(("entry", 0), ("set", 0))
    data(("entry", 0), ("local", 0))
    data(("local", 0), ("add", 0))
    data(("local", 0), ("add", 1))
    data(("add", 0), ("set", 0))
    ctrl(("set", 0), ("log", 0))
}, height: 240pt)

= 单独一个节点

`node` 放进 `graph()` 里才是图的一部分;不放,就是单独摆一个 —— 讲引脚长什么样
时用得着,不用换个函数。

#{
  import graph: *;

  node({
    [Int + Int]
    data_in("Int")[左]
    data_in("Int")[右]
    data_out("Int")[和]
  })
}

= 语法

完整的样子:

```typst
#import "/lib/graph.typ";

graph.graph({
    import graph: *;
    node("entry", {
        [入口]
        ctrl_out()
        data_out("Entity")[玩家]
    })
    node("set", {
        [SetLocal\<Int\>]
        ctrl_in()
        data_in("Int")[值]
    })
    ctrl(("entry", 0), ("set", 0))
    data(("entry", 0), ("set", 0))
})
```

`graph` 收一整块内容。里面的多个语句会被自动 join,所以不用逗号隔开 —— 想用
逗号写成 `graph(node(...), connection(...))` 也可以,两种等价。

== 节点

```typst
node("set", {
    [SetLocal\<Int\>]
    ctrl_in()
    data_in("Int")[值]
    ctrl_out()
})
```

- 头一个位置参数是 `id`,在图里唯一,连接靠它引用;不写也行 —— 只是单独摆一个看看形状的话不必费劲起名,但那样连接就引用不到它
- 第二块是内容,标题写在最前面,后面跟引脚
- 标题和引脚不用分开传 —— 整块一起投影进盒子,盒子量多宽节点就多宽
- 标题是 markup,`<` / `>` 得转义成 `\<` / `\>` —— 不然 `<Int>` 会被当成标签吃掉

== 引脚

```typst
ctrl_in()
data_in("Int")[值]
```

四个:`ctrl_in()` / `ctrl_out()` / `data_in(type)[label]` / `data_out(type)[label]`。

- `type` 是数据类型,控制流没有类型,它的两个函数也不用带。它只在鼠标悬停到
  那个圆点或连线上时浮出来 —— 平时不占地方
- 挂在后面的 `[label]` 是显示在引脚旁边的名字,不写就没有
- 引脚没有 id —— 同一节点里同 `kind` + 同 `dir` 的引脚算一组,连接里写的
  就是组内序号(从 0 起)。`in`/`out` × `ctrl`/`data` 四组各数各的:
  `data` 连线上的 `0` 是那节点的第一个 data 引脚,`1` 是第二个

底下还有个 `pin(dir, kind, type, ..label)` —— 上面四个都只是它的封装,平时不用
直接调。

== 连接

```typst
data(("entry", 0), ("local", 0))
ctrl(("entry", 0), ("set", 0))
```

两个:`ctrl(from, to)` / `data(from, to)`。两端各写 `(节点 id, 组内序号)`,序号
各数各的:起点按 `out` 数,终点按 `in` 数。

底下还有个 `connection(kind, from, to)`,是这两个的共同底座。

= 报错

参数个数不对(少给了、给多了)由 Typst 自己报。写错了内容会在*构建期*报错,并指出是哪一处:

- 引脚的 `dir` / `kind` 写了别的值
- 连接某一端不是 `(节点 id, 组内序号)` 的形状

跨引用的错误 —— 连接指向了不存在的节点、引脚序号越界、节点 `id` 重复 ——
要等整张图收齐了才知道,所以是在*浏览器控制台*用 `console.error` 报的,
报的同样是哪一处。
