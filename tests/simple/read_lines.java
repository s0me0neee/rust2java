import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.stream.LongStream;

// read_line into a buffer that is only read trimmed: `buf = STDIN.readLine();`, and
// `throws IOException` on the methods that read (not on one called from a closure).
class Main {
    static final BufferedReader STDIN = new BufferedReader(new InputStreamReader(System.in));

    static long read_num() {
        String s = "";
        s += STDIN.lines().findFirst().map(l -> l + "\n").orElse("");
        return Long.parseLong(s.strip());
    }

    static Map.Entry<Long, Long> read_pair() throws IOException {
        String s = STDIN.readLine();
        List<Long> v = Arrays.stream(s.strip().split("\\s+")).filter(w -> !w.isEmpty()).map(Long::parseLong).toList();
        return Map.entry(v.get(0), v.get(1));
    }

    static long solve() throws IOException {
        Map.Entry<Long, Long> read_pair = read_pair();
        long a = read_pair.getKey();
        long b = read_pair.getValue();
        return a + b;
    }

    public static void main(String[] args) throws IOException {
        long n = read_num();
        List<Long> xs = LongStream.range(0L, n).boxed().map(unused -> read_num()).toList();
        System.out.println(xs + " " + solve());
    }
}
