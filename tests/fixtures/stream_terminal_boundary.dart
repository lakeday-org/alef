class Delta {
  final String? content;
  final List<String>? toolCalls;
  const Delta(this.content, this.toolCalls);
}

class Choice {
  final String? finishReason;
  final Delta delta;
  const Choice(this.finishReason, this.delta);
}

class Chunk {
  final List<Choice> choices;
  const Chunk(this.choices);
}

const isTrue = true;
void expect(Object? actual, Object? expected) {
  if (actual != expected) throw StateError('$actual != $expected');
}

Chunk chunk([String? reason, String text = '', List<String> tools = const []]) =>
Chunk([Choice(reason, Delta(text, tools))]);

bool boundary(List<Chunk> result) {
  try {
    // GENERATED_ASSERTION ~keep
    return true;
  } on StateError {
    return false;
  }
}

void main() {
  final terminal = chunk('stop');
  const empty = Chunk([]);
  final cases = <(List<Chunk>, bool)>[
    ([terminal], true),
    ([chunk(null, 'text'), terminal], true),
    ([empty, terminal], true),
    ([terminal, empty], false),
    ([terminal, chunk()], false),
    ([terminal, chunk(null, 'late')], false),
    ([terminal, chunk(null, '', ['tool'])], false),
    ([terminal, terminal], false),
    ([chunk(null, 'unterminated')], false),
    ([], false),
  ];
  for (var index = 0; index < cases.length; index++) {
    final (chunks, expected) = cases[index];
    final actual = boundary(chunks);
    if (actual != expected) throw StateError('case $index: $actual != $expected');
  }
  final result = [terminal, empty];
  final doneAt = result.indexWhere((c) => c.choices.any((ch) => ch.finishReason != null));
  final old = doneAt >= 0 && result.skip(doneAt + 1).every((c) => c.choices.every(
    (ch) => (ch.delta.content ?? '').isEmpty && (ch.delta.toolCalls ?? []).isEmpty,
  ));
  if (!old) throw StateError('old predicate must demonstrate an empty-chunk false pass');
  if (boundary(result)) throw StateError('generated strict predicate must reject the same stream');
  print('BOUNDARY_CASES=${cases.length};OLD_PREDICATE_FALSE_PASS=1');
}
