document.addEventListener('mouseover', e => {
    const link = e.target.closest('a[href^="#"]');
    let href;
    if(!link || (href = link.getAttribute('href')) === '#')
        return;
    document.getElementById(href.substring(1)).classList.add('highlight');
});

document.addEventListener('mouseout', e => {
    const link = e.target.closest('a[href^="#"]');
    let href;
    if(!link || (href = link.getAttribute('href')) === '#')
        return;
    document.getElementById(href.substring(1)).classList.remove('highlight');
});
