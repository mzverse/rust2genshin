#import "/lib/lib.typ": *;
#show: template;

#let title = [Local];
#let tag = <tag-local>;

#import "/lib/graph.typ";

#{
    import graph: *;
    node({
        "Local<T>"
        data_out("LocalRef")[setter]
        data_out("PartialMut<T>")[value]
    })
}

局部变量节点

两个出口：setter（`LocalRef`）和值。值交出来的是#link("/dev/ir/kind/partial_mut.html")[可变视图]，读值要过`Copy`节点

设置使用#link(<tag-set-local>)[SetLocal]

除了原生支持的类型，额外支持ADT
