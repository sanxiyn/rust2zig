import json
import operator
import os
import sys

from elastictabstops import Table

def print_table(rows, right=()):
    widths = [max(len(row[i]) for row in rows) for i in range(len(rows[0]))]
    cells = [[cell.rjust(widths[i]) if i in right else cell
              for i, cell in enumerate(row)] for row in rows]
    print(Table(cells).to_spaces(2))

def get_covered(out, name):
    data = json.load(open(os.path.join(out, name + ".json")))["data"][0]
    regions = set()
    for function in data["functions"]:
        filenames = function["filenames"]
        for start_line, start_col, end_line, end_col, count, file_id, *_ in function["regions"]:
            if count > 0:
                regions.add((filenames[file_id], start_line, start_col, end_line, end_col))
    return regions

def short(filename):
    return filename.split("/src/")[-1] if "/src/" in filename else filename

def main(out):
    names = []
    for f in os.listdir(out):
        if f.endswith(".json"):
            names.append(f.removesuffix(".json"))
    names.sort()

    coverage = {name: get_covered(out, name) for name in names}
    union = set().union(*coverage.values())

    def union_without(dropped):
        kept = [name for name in names if name not in dropped]
        return set().union(*(coverage[name] for name in kept)) if kept else set()

    print(f"{len(names)} tests, {len(union)} regions covered")
    print()

    database = []
    for name in names:
        covered = len(coverage[name])
        unique = len(coverage[name] - union_without({name}))
        database.append((name, covered, unique))

    rows = [["test", "covered", "unique"]]
    for name, covered, unique in sorted(database, key=operator.itemgetter(1), reverse=True):
        rows.append([name, str(covered), str(unique)])
    print_table(rows, right=(1, 2))
    print()

    candidates = [name for name, covered, unique in database if not unique]
    if not candidates:
        print("No redundancy")
    else:
        print(f"Candidate: {', '.join(candidates)}")

    dropped = set(candidates)
    while True:
        lost = union - union_without(dropped)
        if not lost:
            break
        keeper = min(dropped, key=lambda name: (-len(coverage[name] & lost), len(coverage[name]), name))
        print(f"  ...but {', '.join(sorted(name for name in dropped if coverage[name] & lost))} keep {len(lost)} regions; keeping {keeper}:")
        for region in sorted(coverage[keeper] & lost):
            print(f"    {short(region[0])}:{region[1]}")
        dropped.remove(keeper)
    if candidates:
        print()

    if dropped:
        print(f"Redundant: {', '.join(sorted(dropped))}")
    elif candidates:
        print("No redundancy")

    thin = [(name, unique) for name, covered, unique in database if 0 < unique <= 5]
    if not thin:
        return
    print()

    print("Nearly redundant:")
    rows = [["test", "unique", "where"]]
    for name, unique in sorted(thin, key=operator.itemgetter(1)):
        only = sorted(coverage[name] - union_without({name}))
        where_list = sorted({f"{short(region[0])}:{region[1]}" for region in only})
        first = True
        for where in where_list:
            if first:
                rows.append([name, str(unique), where])
                first = False
            else:
                rows.append(["", "", where])
    print_table(rows, right=(1,))

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: redundancy.py <report-dir>")
        sys.exit()
    main(sys.argv[1])
