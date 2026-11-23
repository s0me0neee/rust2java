import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import java.util.function.Function;
import java.util.stream.IntStream;

class Main {
    // Position-based iteration: enumerate, zip with a shifted copy, windows, chunks.
    public static void main(String[] args) {
        var v = List.of(3, 1, 4, 1, 5, 9, 2, 6);
        var w = List.of(10, 20, 30);
        List<Integer> diffs = IntStream.range(0, v.size() - 1).mapToObj(j -> v.get(j + 1) - v.get(j)).toList();
        int dot = IntStream.range(0, Math.min(v.size(), w.size())).mapToObj(j -> v.get(j) * w.get(j)).mapToInt(x2 -> x2).sum();
        List<String> tagged = IntStream.range(0, w.size()).mapToObj(i -> i + ":" + w.get(i)).toList();
        boolean sorted = IntStream.range(0, v.size() - 1).allMatch(j -> v.get(j) <= v.get(j + 1));
        System.out.println(diffs + " " + dot + " " + tagged + " " + sorted);
        List<Integer> big_at = IntStream.range(0, v.size()).filter(i -> v.get(i) > 4).boxed().toList();
        int rises = (int) IntStream.range(0, v.size() - 1).filter(j -> v.get(j) < v.get(j + 1)).mapToObj(j -> v.get(j + 1) - v.get(j)).count();
        System.out.println(big_at + " " + rises);

        String s = "hello";
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c == 'l') {
                System.out.print(i + " ");
            }
        }
        System.out.println();

        String text = "a b\nc d e";
        List<String> lines = text.lines().toList();
        for (int row = 0; row < lines.size(); row++) {
            String line = lines.get(row);
            System.out.println(row + ": " + (int) Arrays.stream(line.strip().split("\\s+")).filter(w2 -> !w2.isEmpty()).count());
        }

        for (int j = 0; j + 1 < v.size(); j++) {
            List<Integer> pair = v.subList(j, j + 2);
            System.out.print(pair.get(0) + pair.get(1) + " ");
        }
        System.out.println();
        for (int k = 0; k < v.size(); k += 3) {
            List<Integer> chunk = v.subList(k, Math.min(k + 3, v.size()));
            System.out.print(chunk.size() + " ");
        }
        System.out.println();
        for (int l = 0; l + 1 < v.size(); l++) {
            int a = v.get(l);
            int b = v.get(l + 1);
            if (a > b) {
                System.out.print(a + ">" + b + " ");
            }
        }
        System.out.println();

        // Streams without positions (map entries) count with a plain int.
        var stock = new TreeMap<String, Integer>();
        stock.put("pear", 0);
        stock.put("apple", 3);
        stock.put("fig", 7);
        int i2 = 0;
        for (Map.Entry<String, Integer> item : stock.entrySet()) {
            String name = item.getKey();
            int n = item.getValue();
            System.out.println(i2 + ". " + name + " x" + n);
            i2++;
        }
        int index = 0;
        for (Map.Entry<String, Integer> item2 : stock.entrySet()) {
            int i3 = index++;
            String name2 = item2.getKey();
            int n2 = item2.getValue();
            if (n2 == 0) {
                continue;
            }
            Function<String, String> label = s2 -> "#" + i3 + " " + s2;
            System.out.println(label.apply(name2));
        }
        System.out.println(first_empty(stock));
    }

    static int first_empty(TreeMap<String, Integer> stock) {
        int index = 0;
        for (Map.Entry<String, Integer> item : stock.entrySet()) {
            int i = index++;
            int n = item.getValue();
            if (n == 0) {
                return i;
            } else {
                continue;
            }
        }
        return Integer.MAX_VALUE;
    }
}
