set script (realpath (status --current-filename))
if pgrep -f $script >/dev/null 2>/dev/null
    exit 0
end

function tempfile
    if command -v mktemp >/dev/null 2>/dev/null
        command mktemp
    else
        command tempfile
    end
end

set MONITOR_TEMP_FILE (tempfile)
set MONITOR_PID_FILE (tempfile)

function duraMonitor
    pkill -P (cat $MONITOR_PID_FILE) 2>/dev/null
    pkill (cat $MONITOR_PID_FILE) 2>/dev/null

    echo '
    set repos (cat ~/.config/dura/config.json | jq -rc \'.repos | keys | join("§")\' 2>/dev/null)
    set pollingSeconds (cat ~/.config/dura/config.json | jq -r ".pollingSeconds // 5")

    fswatch -e .git -0 -l $pollingSeconds -r (string split "§" -- $repos) | while read -l -z path
        cd $path 2>/dev/null || cd (dirname $path) && cd (git rev-parse --show-toplevel) && dura capture
    end
    ' >$MONITOR_TEMP_FILE

    fish $MONITOR_TEMP_FILE &
    jobs -p >$MONITOR_PID_FILE
end


duraMonitor
fswatch -0 -l 3 ~/.config/dura/config.json | while read -l -z path
    duraMonitor
end
