#import "/lib/lib.typ": *;
#show: template;

#let title = [nop];

#import "/lib/graph.typ";

#{
    import graph: *;
    node({
        [nop]
        ctrl_in()
        ctrl_out()
    })
}

不进行任何操作，直接执行下一节点

此节点的意义是提供一个可连接的控制流，以便生成节点图

在优化时应全部去除该节点
