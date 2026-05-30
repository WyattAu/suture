#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

MD2HTML="$SCRIPT_DIR/md2html.awk"
TEMPLATE="$SCRIPT_DIR/template.html"

SKIP_NAMES=("index" "demo")

get_title() {
    local file="$1"
    # Handle YAML frontmatter: ---\ntitle: "..." \n---
    local first_line
    first_line="$(head -1 "$file" 2>/dev/null)"
    if [ "$first_line" = "---" ]; then
        # Extract title from frontmatter block
        local title
        title="$(awk '/^---/{n++; next} n==1 && /^title:/ {sub(/^title:[[:space:]]*["'"'"']?/, ""); sub(/["'"'"']?[[:space:]]*$/, ""); print; exit}' "$file" 2>/dev/null)"
        if [ -n "$title" ]; then
            echo "$title"
            return
        fi
    fi
    # Fallback: first markdown heading
    head -1 "$file" 2>/dev/null | sed 's/^# *//;s/ *$//' || echo "Documentation"
}

get_nav_group() {
    local base="$1"
    local dir="${2:-.}"
    # Blog subdirectory gets its own group
    if [ "$dir" = "./blog" ] || [ "$dir" = "blog" ]; then
        echo "Blog"
        return
    fi
    case "$base" in
        quickstart|getting_started)         echo "Getting Started" ;;
        why-suture|semantic-merge|comparing-with-git|comparison)
                                              echo "Core Concepts" ;;
        cli-reference|api_reference)         echo "Reference" ;;
        git_merge_driver|merge-driver-guide|driver_sdk)
                                              echo "Merge Drivers" ;;
        ide-integration|github-action)       echo "Integration" ;;
        document-authors|video-editors|video-merge-guide|data-science)
                                              echo "Guides" ;;
        onboarding-*)                        echo "Onboarding" ;;
        hub|desktop-build|wasm-feasibility)  echo "Platform" ;;
        release-notes|shipping-checklist|performance)
                                              echo "Development" ;;
        *)                                   echo "Other" ;;
    esac
}

get_display_title() {
    local title="$1"
    local name="$2"
    if [ "$title" = "Documentation" ] || [ -z "$title" ]; then
        echo "$name" | sed 's/-/ /g; s/_/ /g; s/\b\(.\)/\u\1/g'
    else
        echo "$title"
    fi
}

should_skip() {
    local name="$1"
    for s in "${SKIP_NAMES[@]}"; do
        if [ "$name" = "$s" ]; then
            return 0
        fi
    done
    return 1
}

generate_nav() {
    local current="$1"
    shift
    local files=("$@")

    local -a groups=()
    local -a names=()
    local -a titles=()
    local -a links=()
    local -a link_prefixes=()

    for ((i=0; i<${#files[@]}; i++)); do
        local f="${files[$i]}"
        local base="$(basename "$f" .md)"
        local dir="$(dirname "$f")"
        if [ "$dir" = "." ]; then
            local link="${base}.html"
            local link_prefix=""
        else
            local link="${dir}/${base}.html"
            local link_prefix="../"
        fi
        local grp="$(get_nav_group "$base" "$dir")"
        local ttl="$(get_display_title "${title_cache[$base]}" "$base")"
        groups+=("$grp")
        names+=("$base")
        titles+=("$ttl")
        links+=("$link")
        link_prefixes+=("$link_prefix")
    done

    declare -A group_files_idx
    for ((i=0; i<${#groups[@]}; i++)); do
        group_files_idx["${groups[$i]}"]+="$i "
    done

    local -a order=("Getting Started" "Core Concepts" "Reference" "Merge Drivers" "Integration" "Guides" "Onboarding" "Platform" "Blog" "Development" "Other")

    echo '<a href="${link_prefix}index.html" class="sidebar-home">&larr; Back to Home</a>'

    for grp in "${order[@]}"; do
        [ -z "${group_files_idx[$grp]+x}" ] && continue

        echo "<div class=\"nav-group\">"
        echo "<div class=\"nav-group-title\">$grp</div>"

        for idx in ${group_files_idx[$grp]}; do
            local cls="nav-item"
            if [ "${names[$idx]}" = "$current" ]; then
                cls="$cls active"
            fi
            local html_name="${link_prefixes[$idx]}${links[$idx]}"
            echo "<a href=\"$html_name\" class=\"$cls\">${titles[$idx]}</a>"
        done
        echo "</div>"
    done
}

fill_template() {
    local title="$1" content="$2" nav="$3"

    export _TPL_TITLE="$title"
    export _TPL_CONTENT="$content"
    export _TPL_NAV="$nav"

    awk '
    BEGIN { RS = sprintf("%c", 0) }
    {
        s = $0
        while ((i = index(s, "{{TITLE}}")) > 0)
            s = substr(s, 1, i-1) ENVIRON["_TPL_TITLE"] substr(s, i+9)
        while ((i = index(s, "{{CONTENT}}")) > 0)
            s = substr(s, 1, i-1) ENVIRON["_TPL_CONTENT"] substr(s, i+11)
        while ((i = index(s, "{{NAV}}")) > 0)
            s = substr(s, 1, i-1) ENVIRON["_TPL_NAV"] substr(s, i+7)
        printf "%s", s
    }
    ' "$TEMPLATE"
}

# Strip YAML frontmatter (--- ... ---) from a markdown file.
# Outputs the file content with frontmatter removed, ready for md2html.
strip_frontmatter() {
    local file="$1"
    local first_line
    first_line="$(head -1 "$file" 2>/dev/null)"
    if [ "$first_line" = "---" ]; then
        awk 'BEGIN{skip=1} /^---$/{skip=0; next} !skip{print}' "$file"
    else
        cat "$file"
    fi
}

[ -f "$SCRIPT_DIR/md2html.sh" ] && mv "$SCRIPT_DIR/md2html.sh" "$SCRIPT_DIR/md2html.awk"

mapfile -t md_files < <(find . -name '*.md' | sort)

if [ ${#md_files[@]} -eq 0 ]; then
    echo "No markdown files found in $SCRIPT_DIR"
    exit 1
fi

echo "Building documentation site..."
echo "Found ${#md_files[@]} markdown files"

declare -A title_cache
for md_file in "${md_files[@]}"; do
    base="$(basename "$md_file" .md)"
    title_cache["$base"]="$(get_title "$md_file")"
done

base_nav="$(generate_nav "" "${md_files[@]}")"

converted=0
skipped=0

for md_file in "${md_files[@]}"; do
    base="$(basename "$md_file" .md)"
    dir="$(dirname "$md_file")"
    if [ "$dir" = "." ]; then
        html_file="${base}.html"
    else
        html_file="${dir}/${base}.html"
        mkdir -p "$dir"
    fi

    if should_skip "$base"; then
        echo "  SKIP $md_file (protected: $html_file exists)"
        skipped=$((skipped + 1))
        continue
    fi

    if [ ! -f "$md_file" ]; then
        echo "  WARN $md_file not found, skipping"
        continue
    fi

    echo "  CONV $md_file -> $html_file"

    title="$(get_title "$md_file")"
    content="$(strip_frontmatter "$md_file" | awk -f "$MD2HTML")"
    nav="$(echo "$base_nav" | sed "s/class=\"nav-item\"/class=\"nav-item active\"/; t; b; :a; n; s/class=\"nav-item\"/class=\"nav-item active\"/; t; b")"

    fill_template "$title" "$content" "$nav" > "$html_file"
    converted=$((converted + 1))
done

echo ""
echo "Done. $converted files converted, $skipped skipped."

# Generate sitemap.xml
echo "Generating sitemap.xml..."
BASE_URL="https://suture.dev"
SITEMAP='<?xml version="1.0" encoding="UTF-8"?>'
SITEMAP="$SITEMAP
<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">
  <url>
    <loc>${BASE_URL}/</loc>
    <changefreq>weekly</changefreq>
    <priority>1.0</priority>
  </url>"

for md_file in "${md_files[@]}"; do
    base="$(basename "$md_file" .md)"
    dir="$(dirname "$md_file")"
    if [ "$dir" = "." ]; then
        html_file="${base}.html"
        url_path="/${base}.html"
    else
        html_file="${dir}/${base}.html"
        url_path="/${dir}/${base}.html"
    fi
    if should_skip "$base"; then
        continue
    fi
    if [ -f "$html_file" ]; then
        SITEMAP="$SITEMAP
  <url>
    <loc>${BASE_URL}${url_path}</loc>
    <changefreq>weekly</changefreq>
    <priority>0.5</priority>
  </url>"
    fi
done

SITEMAP="$SITEMAP
</urlset>"

echo -e "$SITEMAP" > sitemap.xml
echo "Sitemap generated."
