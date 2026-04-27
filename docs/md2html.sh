#!/usr/bin/env bash
set -euo pipefail

if [ $# -lt 1 ]; then
    echo "Usage: $0 <file.md>" >&2
    exit 1
fi

awk '
function esc(s) {
    gsub(/&/, "\\&amp;", s)
    gsub(/</, "\\&lt;", s)
    gsub(/>/, "\\&gt;", s)
    return s
}

function fmt(s,    p, m, c, u, t) {
    gsub(/\.md\)/, ".html)", s)
    gsub(/\.md#/, ".html#", s)

    while (match(s, /`[^`]+`/)) {
        p = substr(s, 1, RSTART - 1)
        m = substr(s, RSTART, RLENGTH)
        c = m; sub(/^`/, "", c); sub(/`$/, "", c)
        c = esc(c)
        s = p "<code>" c "</code>" substr(s, RSTART + RLENGTH)
    }

    while (match(s, /!\[[^]]*\]\([^)]*\)/)) {
        p = substr(s, 1, RSTART - 1)
        m = substr(s, RSTART, RLENGTH)
        a = m; sub(/^!\[/, "", a); sub(/\].*/, "", a)
        u = m; sub(/^.*\]\(/, "", u); sub(/\)$/, "", u)
        s = p "<img src=\"" u "\" alt=\"" a "\" loading=\"lazy\">" substr(s, RSTART + RLENGTH)
    }

    while (match(s, /\[[^]]*\]\([^)]*\)/)) {
        p = substr(s, 1, RSTART - 1)
        m = substr(s, RSTART, RLENGTH)
        t = m; sub(/^\[/, "", t); sub(/\].*/, "", t)
        u = m; sub(/^.*\]\(/, "", u); sub(/\)$/, "", u)
        s = p "<a href=\"" u "\">" t "</a>" substr(s, RSTART + RLENGTH)
    }

    while (match(s, /\*\*[^*]+\*\*/)) {
        p = substr(s, 1, RSTART - 1)
        m = substr(s, RSTART, RLENGTH)
        c = m; sub(/^\*\*/, "", c); sub(/\*\*$/, "", c)
        s = p "<strong>" c "</strong>" substr(s, RSTART + RLENGTH)
    }

    while (match(s, /\*[^*]+\*/)) {
        p = substr(s, 1, RSTART - 1)
        m = substr(s, RSTART, RLENGTH)
        c = m; sub(/^\*/, "", c); sub(/\*$/, "", c)
        s = p "<em>" c "</em>" substr(s, RSTART + RLENGTH)
    }

    while (match(s, /~~[^~]+~~/)) {
        p = substr(s, 1, RSTART - 1)
        m = substr(s, RSTART, RLENGTH)
        c = m; sub(/^~~/, "", c); sub(/~~$/, "", c)
        s = p "<del>" c "</del>" substr(s, RSTART + RLENGTH)
    }

    return s
}

function close_all(    t) {
    if (in_p) { print "</p>"; in_p = 0 }
    if (in_ul) { print "</ul>"; in_ul = 0 }
    if (in_ol) { print "</ol>"; in_ol = 0 }
    if (in_bq) { print "</blockquote>"; in_bq = 0 }
    if (in_table) {
        if (tbody_open) print "</tbody>"
        print "</table>"
        in_table = 0; tbody_open = 0
    }
}

function close_lists() {
    if (in_ul) { print "</ul>"; in_ul = 0 }
    if (in_ol) { print "</ol>"; in_ol = 0 }
}

BEGIN {
    in_code = 0; in_table = 0; tbody_open = 0
    in_ul = 0; in_ol = 0; in_bq = 0; in_p = 0
}

{
    line = $0
    stripped = line
    gsub(/^[[:space:]]+/, "", stripped)

    if (stripped ~ /^```/) {
        if (in_code) {
            print "</code></pre>"
            in_code = 0
        } else {
            close_all()
            lang = stripped
            sub(/^```/, "", lang)
            gsub(/^[[:space:]]+/, "", lang)
            if (lang != "") printf "<pre><code class=\"language-%s\">", lang
            else print "<pre><code>"
            in_code = 1
        }
        next
    }

    if (in_code) {
        print esc(line)
        next
    }

    if (stripped == "") {
        if (in_p) { print "</p>"; in_p = 0 }
        if (in_bq) { print "</blockquote>"; in_bq = 0 }
        if (in_table) {
            if (tbody_open) print "</tbody>"
            print "</table>"
            in_table = 0; tbody_open = 0
        }
        next
    }

    if (in_table && stripped !~ /^\|/) {
        if (tbody_open) print "</tbody>"
        print "</table>"
        in_table = 0; tbody_open = 0
    }

    if (stripped ~ /^#{1,6}[[:space:]]/) {
        close_all()
        h = stripped; lv = 0
        while (substr(h, 1, 1) == "#") { lv++; h = substr(h, 2) }
        gsub(/^[[:space:]]+/, "", h)
        h = fmt(h)
        if (lv == 1) {
            print "<h1>" h "</h1>"
        } else {
            id = h
            gsub(/<[^>]+>/, "", id)
            gsub(/[^a-zA-Z0-9 _-]/, "", id)
            gsub(/ +/, "-", id)
            id = tolower(id)
            printf "<h%d id=\"%s\">%s</h%d>\n", lv, id, h, lv
        }
        next
    }

    if (stripped ~ /^(-{3,}|\*{3,}|_{3,})[[:space:]]*$/) {
        close_all()
        print "<hr>"
        next
    }

    if (stripped ~ /^\|/) {
        if (!in_table) {
            close_all()
            in_table = 1; tbody_open = 0
            delete aligns
        }

        row = stripped
        sub(/^\|/, "", row)
        sub(/\|[[:space:]]*$/, "", row)
        nc = split(row, cells, /\|/)

        is_sep = 1
        for (i = 1; i <= nc; i++) {
            gsub(/^[[:space:]]+|[[:space:]]+$/, "", cells[i])
            if (cells[i] !~ /^[-:]+$/) { is_sep = 0; break }
        }

        if (is_sep && !tbody_open) {
            for (i = 1; i <= nc; i++) {
                c = cells[i]
                if (c ~ /^:.+:$/) aligns[i] = "center"
                else if (c ~ /^:.+/) aligns[i] = "left"
                else if (c ~ /:+$/) aligns[i] = "right"
                else aligns[i] = ""
            }
            print "</tr></thead>"
            print "<tbody>"
            tbody_open = 1
            next
        }

        if (!tbody_open) {
            printf "<thead><tr>"
            for (i = 1; i <= nc; i++) {
                gsub(/^[[:space:]]+|[[:space:]]+$/, "", cells[i])
                printf "<th>%s</th>", fmt(cells[i])
            }
            next
        }

        printf "<tr>"
        for (i = 1; i <= nc; i++) {
            gsub(/^[[:space:]]+|[[:space:]]+$/, "", cells[i])
            a = ""
            if (i in aligns && aligns[i] != "") a = " style=\"text-align:" aligns[i] "\""
            printf "<td%s>%s</td>", a, fmt(cells[i])
        }
        print "</tr>"
        next
    }

    if (stripped ~ /^>/) {
        close_lists()
        if (in_p) { print "</p>"; in_p = 0 }
        bq_text = stripped
        sub(/^>[[:space:]]?/, "", bq_text)
        if (!in_bq) { print "<blockquote>"; in_bq = 1 }
        printf "<p>%s</p>\n", fmt(bq_text)
        next
    }

    if (in_bq) {
        print "</blockquote>"
        in_bq = 0
    }

    if (stripped ~ /^[-*][[:space:]]/) {
        if (in_p) { print "</p>"; in_p = 0 }
        if (in_ol) { print "</ol>"; in_ol = 0 }
        if (!in_ul) { print "<ul>"; in_ul = 1 }
        item = stripped
        sub(/^[-*][[:space:]]+/, "", item)
        printf "<li>%s</li>\n", fmt(item)
        next
    }

    if (stripped ~ /^[0-9]+\.[[:space:]]/) {
        if (in_p) { print "</p>"; in_p = 0 }
        if (in_ul) { print "</ul>"; in_ul = 0 }
        if (!in_ol) { print "<ol>"; in_ol = 1 }
        item = stripped
        sub(/^[0-9]+\.[[:space:]]+/, "", item)
        printf "<li>%s</li>\n", fmt(item)
        next
    }

    close_lists()

    if (!in_p) {
        printf "<p>"
        in_p = 1
    } else {
        printf " "
    }
    printf "%s", fmt(line)
}

END {
    if (in_p) print "</p>"
    if (in_ul) print "</ul>"
    if (in_ol) print "</ol>"
    if (in_bq) print "</blockquote>"
    if (in_table) {
        if (tbody_open) print "</tbody>"
        print "</table>"
    }
}
' "$1"
