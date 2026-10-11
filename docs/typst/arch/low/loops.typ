#import "/lib/lib.typ": *;
#let title = [循环];
#show: template;

#set raw(lang: "rust");

#import "/lib/graph.typ";

节点图里没有「跳回上面」这种操作（控制流不允许成环），循环要靠原生的循环节点实现。有两种。

= 有限循环

#{
    import graph: *;
    node({
        [有限循环]
        ctrl_in()
        ctrl_in[退出循环]
        ctrl_out[循环体]
        ctrl_out[循环结束]
        data_in("Int")[begin]
        data_in("Int")[end]
        data_out("Int")[当前值]
    })
}

它*只储存*一个东西：当前循环值。进入循环时，当前循环值被设成*循环起始值*。

*循环终止值*每次都会重新获取，*不储存*。所以终止值引脚连着的节点会被反复求值。

用 `<=` 判断：循环包含上下界。等价于

```cpp
for (int i = begin; i <= end; i++)
```

注意当上界是 `i32::MAX` 时，这个循环*无法正常结束*（`i` 递增会溢出回绕，永远满足条件）。

= 列表迭代循环

#{
    import graph: *;
    node({
        [列表迭代循环]
        ctrl_in()
        ctrl_in[退出循环]
        ctrl_out[循环体]
        ctrl_out[循环结束]
        data_in("List<T>")[列表]
        data_out("T")[当前值]
    })
}

只储存当前索引

*不储存元素，也不储存列表长度* —— 每次用到元素或长度时都重新求一遍，等价于

```cpp
for (int i = 0; i < list.size(); i++) {
    auto &element = list[i];
    // body
}
```

所以循环体里每次都用 `list[i]` / `list.size()` 的值重新取，而不是一开始取一份存下来。

它属于「有控制流的节点」，但出参的处理是个例外 —— 见「数据流与所有权」。
