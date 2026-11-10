if [ "${SHELL##*/}" = bash ] && [ -r "$AEGIS_WSL_BASH" ]; then
    AEGIS_SHELL_LOGIN=1 exec "$SHELL" --init-file "$AEGIS_WSL_BASH" -i
fi
exec "${SHELL:-/bin/sh}" -l
