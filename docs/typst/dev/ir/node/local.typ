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
        data_out("T")[value]
    })
}

局部变量节点

值可直接连接获取，设置使用#link(<tag-set-local>)[SetLocal]

除了原生支持的类型，额外支持ADT
