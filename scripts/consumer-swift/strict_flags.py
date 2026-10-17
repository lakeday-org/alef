"""Remove precisely the documented source-level allowance, preserving other flags."""

import shlex

ALLOWANCE = "link-arg=-Wl,-undefined,dynamic_lookup"


def strict_flags(environment, configured):
    if environment.get("CARGO_ENCODED_RUSTFLAGS") is not None:
        value = environment["CARGO_ENCODED_RUSTFLAGS"]
        flags = value.split("\x1f") if value else []
    elif environment.get("RUSTFLAGS") is not None:
        flags = shlex.split(environment["RUSTFLAGS"])
    else:
        flags = list(configured)
    result = []
    removed = 0
    index = 0
    while index < len(flags):
        if flags[index : index + 2] == ["-C", ALLOWANCE]:
            removed += 1
            index += 2
        elif flags[index] == "-C" + ALLOWANCE:
            removed += 1
            index += 1
        else:
            if "dynamic_lookup" in flags[index]:
                raise ValueError("unrecognized global allowance spelling requires explicit review")
            result.append(flags[index])
            index += 1
    fatal = ["-C", "link-arg=-Wl,-fatal_warnings"]
    if not any(result[i : i + 2] == fatal for i in range(len(result))):
        result.extend(fatal)
    return result, removed
