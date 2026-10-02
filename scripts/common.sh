# Shared by the scripts: the data directory of the program and small helpers.

APP_ID=wl-screen-translate
# The directory the program reads its models from; DATA overrides it.
DATA=${DATA:-${XDG_DATA_HOME:-$HOME/.local/share}/$APP_ID}

die() {
    echo "$0: $*" >&2
    exit 1
}

need() {
    for tool in "$@"; do
        command -v "$tool" >/dev/null || die "needs $tool"
    done
}
