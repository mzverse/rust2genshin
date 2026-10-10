#import "/lib/lib.typ": *

#let headings = state("headings", ())
#let headingPath = state("headingPath", ())

#let template(content) = [
    #headings.update(());
    #headingPath.update(());
    #show heading: x => {
        headings.update(s => {
            s.push(x);
            return s;
        });
        headingPath.update(s => {
            s = s.slice(0, x.level -1);
            s.push(contentToString(x.body));
            return s;
        });
        context html_elem("section", x, attrs: ("id": headingPath.get().join("-"))); // TODO
    }
    #content
]

#custom_element(() => {
    import_style("/lib/css/outline.css");
    import_script("/lib/js/outline.js");
})[]
#html_elem("button", attrs: (class: "outline-toggle"))[☰];
#context {
    let result = ();
    let path = ();
    for i in headings.get() + (heading(level: 1, "EOF"),) {
        path = path.slice(0, i.level -1);
        path.push(contentToString(i.body));
        while result.len()>1 and result.first().at(0) < result.last().at(0) and result.last().at(0) >= i.level {
            let j = 0;
            while result.at(j).at(0) < result.last().at(0) {
                j += 1;
            }
            result.at(j - 1) += result.slice(j);
            result = result.slice(0, j);
        }
        result.push((i.level, link("#"+path.join("-"), i.body)));
    };
    result.pop();
    let f(a) = {
        return list(..a.map(c => if type(c) == content { c } else [
            #c.at(1);
            #f(c.slice(2));
        ]));
    };
    return html_elem("nav", attrs: (class: "outline"), 
        html_elem("button", attrs: (class: "outline-close"))[✕] + f(result));
};
