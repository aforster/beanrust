use regex::Regex;
pub struct StatementIterator<'a> {
    data: &'a str,

    line_iterator: LineIterator<'a>,

    new_statement_matcher: Regex,
    new_multiline_statement_matcher: Regex,

    state: IteratorState,
}

enum IteratorState {
    SearchingNextStart,
    ReadingMultiline(usize), // position of the start of the multiline entry
    FinishedMultilineFoundSingle((usize, usize)), // (start, end) of the next single/multiline entry
}

struct LineIterator<'a> {
    data: &'a str,
    position: usize,
    size: usize,
}

impl<'a> StatementIterator<'a> {
    pub fn new(data: &'a str) -> Self {
        let new_statement_matcher = regex::Regex::new(r"^\d{4}-\d{2}-\d{2}.*").unwrap();
        let new_multiline_statement_matcher = regex::Regex::new(r"^.* \*.*").unwrap();

        StatementIterator {
            data,
            line_iterator: LineIterator::new(data),
            new_statement_matcher,
            new_multiline_statement_matcher,
            state: IteratorState::SearchingNextStart,
        }
    }
}

impl<'a> LineIterator<'a> {
    pub fn new(data: &'a str) -> Self {
        let size = data.len();
        LineIterator {
            data,
            position: 0,
            size,
        }
    }
}
impl<'a> Iterator for LineIterator<'a> {
    type Item = (usize, usize); // (start, end) positions of the line in the original string
    fn next(&mut self) -> Option<(usize, usize)> {
        if self.position >= self.size {
            return None;
        }
        let start = self.position;
        if let Some(pos) = &self.data[start..].find('\n') {
            self.position += pos + 1; // Move past the newline character
            Some((start, start + pos))
        } else {
            // Last line without a newline
            self.position = self.size; // Move to the end
            Some((start, self.size))
        }
    }
}
impl<'a> Iterator for StatementIterator<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        // first match only handles searching next start. In case its a multiline, we need
        // to do more work later on.
        match self.state {
            IteratorState::SearchingNextStart => {
                loop {
                    let (start, end) = self.line_iterator.next()?;
                    let line = &self.data[start..end].trim();
                    if skip_line(line) {
                        continue;
                    }
                    if self.new_multiline_statement_matcher.is_match(line) {
                        self.state = IteratorState::ReadingMultiline(start);
                        // Break out of loop & goto multiline handling after this if statement.
                        break;
                    }

                    if self.new_statement_matcher.is_match(line) {
                        // state remains SearchingNextStart
                        return Some(line);
                    } else {
                        panic!("Unhandled line: {}", line);
                    }
                }
            }
            _ => {}
        }

        match self.state {
            IteratorState::SearchingNextStart => unreachable!(),
            IteratorState::ReadingMultiline(start_pos) => {
                let mut end_pos = start_pos;
                loop {
                    let (line_start, line_end) = match self.line_iterator.next() {
                        Some(l) => l,
                        None => {
                            // End of data reached, return the multiline entry.
                            self.state = IteratorState::SearchingNextStart;
                            return Some(&self.data[start_pos..self.data.len()]);
                        }
                    };
                    let line = &self.data[line_start..line_end].trim();
                    if skip_line(line) {
                        continue;
                    }
                    // if we find either a new single, or a multi line entry, then we are finished with the current entry
                    if self.new_multiline_statement_matcher.is_match(line) {
                        self.state = IteratorState::ReadingMultiline(line_start);
                        return Some(&self.data[start_pos..end_pos]);
                    }

                    if self.new_statement_matcher.is_match(line) {
                        self.state =
                            IteratorState::FinishedMultilineFoundSingle((line_start, line_end));
                        return Some(&self.data[start_pos..end_pos]);
                    }

                    // Continue reading the multiline entry.
                    end_pos = line_end;
                }
            }
            IteratorState::FinishedMultilineFoundSingle((start, end)) => {
                self.state = IteratorState::SearchingNextStart;
                return Some(&self.data[start..end]);
            }
        }
    }
}

fn skip_line(line: &str) -> bool {
    line.is_empty() || line.starts_with(';') || line.starts_with('#')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_statement_iterator() -> Result<(), String> {
        let data = "
        2017-12-01 commodity AMD
2024-10-03 balance Assets:Depot:Cash 0 CHF
; comment

2024-10-04 *
; comment in transaction
  Assets:Depot:Cash   2100 CHF
  Assets:Foo -500 CHF
  Income:Salary -1600 CHF
2017-12-06 commodity AMD
2024-10-04 *
foo bar
2024-10-04 *
foo bar3";
        let mut iterator = StatementIterator::new(data);
        assert_eq!(iterator.next(), Some("2017-12-01 commodity AMD"));
        assert_eq!(
            iterator.next(),
            Some("2024-10-03 balance Assets:Depot:Cash 0 CHF")
        );
        assert_eq!(
            iterator.next(),
            Some(
                "2024-10-04 *
; comment in transaction
  Assets:Depot:Cash   2100 CHF
  Assets:Foo -500 CHF
  Income:Salary -1600 CHF"
            )
        );
        assert_eq!(iterator.next(), Some("2017-12-06 commodity AMD"));
        assert_eq!(iterator.next(), Some("2024-10-04 *\nfoo bar"));
        assert_eq!(iterator.next(), Some("2024-10-04 *\nfoo bar3"));

        assert_eq!(iterator.next(), None);
        Ok(())
    }

    #[test]
    fn test_line_iterator() -> Result<(), String> {
        let mut iterator = LineIterator::new("");
        assert_eq!(iterator.next(), None);

        let data = "foo\n\nbar\n";
        let results: Vec<(usize, usize)> = LineIterator::new(data).collect();
        assert_eq!(results, [(0, 3), (4, 4), (5, 8)]);
        assert_eq!(&data[results[0].0..results[0].1], "foo");
        assert_eq!(&data[results[1].0..results[1].1], "");
        assert_eq!(&data[results[2].0..results[2].1], "bar");

        Ok(())
    }
}
