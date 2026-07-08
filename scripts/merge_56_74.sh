#!/usr/bin/env bash
set -euo pipefail

cd /Users/takumi/apps/gqlrs

git checkout main >/dev/null 2>&1 || true
git pull origin main

for pr in 56 57 58 59 60 62 63 64 65 66 67 68 69 70 71 72 73 74; do
    echo "=== PR #$pr ==="
    git checkout main >/dev/null 2>&1
    gh pr checkout "$pr" --force
    branch=$(git rev-parse --abbrev-ref HEAD)
    echo "Checked out $branch"

    if git rebase main; then
        echo "Rebased cleanly"
    else
        echo "CONFLICT on PR #$pr (branch $branch)"
        git rebase --abort || true
        echo "$pr $branch" >> /tmp/merge_conflicts_56_74.txt
        continue
    fi

    if git push --force-with-lease; then
        echo "Pushed"
    else
        echo "PUSH FAILED on PR #$pr"
        echo "$pr $branch push_failed" >> /tmp/merge_conflicts_56_74.txt
        continue
    fi

    sleep 5

    merged=0
    for attempt in 1 2 3; do
        if gh pr merge "$pr" --squash --delete-branch=false; then
            merged=1
            break
        fi
        echo "Merge attempt $attempt failed for PR #$pr, retrying..."
        sleep 5
    done

    if [ "$merged" -eq 0 ]; then
        echo "MERGE FAILED on PR #$pr"
        echo "$pr $branch merge_failed" >> /tmp/merge_conflicts_56_74.txt
        continue
    fi

    echo "Merged PR #$pr"
    git checkout main
    git pull origin main
done

echo "=== Done ==="
if [ -f /tmp/merge_conflicts_56_74.txt ]; then
    echo "Conflicts/failures:"
    cat /tmp/merge_conflicts_56_74.txt
fi
