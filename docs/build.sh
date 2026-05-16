#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

MD2HTML="$SCRIPT_DIR/md2html.sh"
TEMPLATE="$SCRIPT_DIR/template.html"

SKIP_NAMES=("index" "demo")

get_title() {
    head -1 "$1" 2>/dev/null | sed 's/^# *//;s/ *$//' || echo "Documentation"
}

get_nav_group() {
    case "$1" in
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

    local prev_group=""
    local -a sorted_indices=()
    local -a groups=()
    local -a names=()
    local -a titles=()

    for ((i=0; i<${#files[@]}; i++)); do
        local f="${files[$i]}"
        local base="$(basename "$f" .md)"
        local grp="$(get_nav_group "$base")"
        local ttl="$(get_display_title "$(get_title "$f")" "$base")"
        groups+=("$grp")
        names+=("$base")
        titles+=("$ttl")
    done

    local -a order=("Getting Started" "Core Concepts" "Reference" "Merge Drivers" "Integration" "Guides" "Onboarding" "Platform" "Development" "Other")

    echo '<a href="index.html" class="sidebar-home">&larr; Back to Home</a>'

    for grp in "${order[@]}"; do
        local found=0
        for ((i=0; i<${#groups[@]}; i++)); do
            if [ "${groups[$i]}" = "$grp" ]; then
                found=1
                break
            fi
        done
        [ "$found" = 0 ] && continue

        echo "<div class=\"nav-group\">"
        echo "<div class=\"nav-group-title\">$grp</div>"
        for ((i=0; i<${#groups[@]}; i++)); do
            if [ "${groups[$i]}" = "$grp" ]; then
                local cls="nav-item"
                if [ "${names[$i]}" = "$current" ]; then
                    cls="$cls active"
                fi
                local html_name="${names[$i]}.html"
                echo "<a href=\"$html_name\" class=\"$cls\">${titles[$i]}</a>"
            fi
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

chmod +x "$MD2HTML"

mapfile -t md_files < <(find . -name '*.md' | sort)

if [ ${#md_files[@]} -eq 0 ]; then
    echo "No markdown files found in $SCRIPT_DIR"
    exit 1
fi

echo "Building documentation site..."
echo "Found ${#md_files[@]} markdown files"

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
    content="$(bash "$MD2HTML" "$md_file")"
    nav="$(generate_nav "$base" "${md_files[@]}")"

    fill_template "$title" "$content" "$nav" > "$html_file"
    converted=$((converted + 1))
done

echo ""
echo "Done. $converted files converted, $skipped skipped."
