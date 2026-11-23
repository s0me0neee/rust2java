import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.stream.Collectors;

// Tuples the program builds: an aliased grid position, weighted edges, a set of points; map items stay entries.
class Main {
    static List<Pos> neighbors(Pos p, int rows, int cols) {
        int r = p.r();
        int c = p.c();
        var out = new ArrayList<Pos>();
        if (r > 0) {
            out.add(new Pos(r - 1, c));
        }
        if (c > 0) {
            out.add(new Pos(r, c - 1));
        }
        if (r + 1 < rows) {
            out.add(new Pos(r + 1, c));
        }
        if (c + 1 < cols) {
            out.add(new Pos(r, c + 1));
        }
        return out;
    }

    public static void main(String[] args) {
        var grid = List.of("..#", "...", "#..");
        Pos start = new Pos(0, 0);
        var dist = new HashMap<Pos, Integer>();
        var q = new ArrayDeque<Pos>();
        dist.put(start, 0);
        q.addLast(start);
        while (!q.isEmpty()) {
            Pos cur = q.pollFirst();
            for (Pos n : neighbors(cur, 3, 3)) {
                int r = n.r();
                int c = n.c();
                if ((int) grid.get(r).charAt(c) == (int) '#' || dist.containsKey(n)) {
                    continue;
                }
                dist.put(n, dist.get(cur) + 1);
                q.addLast(n);
            }
        }
        System.out.println(dist.get(new Pos(2, 2)));

        var edges = new ArrayList<Edge>(List.of(new Edge(0, 1, 5L), new Edge(1, 2, 3L), new Edge(0, 2, 9L)));
        edges.sort(Comparator.comparingLong((Edge tuple) -> {
            long w = tuple.w();
            return w;
        }));
        for (Edge item : edges) {
            int from = item.from();
            int to = item.to();
            long weight = item.w();
            System.out.println(from + "->" + to + " " + weight);
        }
        long total = edges.stream().map(e -> e.w()).mapToLong(x -> x).sum();
        int heavy = (int) edges.stream().filter(tuple -> {
            long w = tuple.w();
            return w > 4L;
        }).count();
        Edge edge = edges.get(0);
        System.out.println(total + " " + heavy + " " + ("(" + edge.from() + ", " + edge.to() + ", " + edge.w() + ")"));

        var seen = new HashSet<Pair>();
        seen.add(new Pair(1, 2));
        System.out.println(seen.contains(new Pair(1, 2)));

        HashMap<String, Integer> scores = new ArrayList<>(List.of(Map.entry("a", 1), Map.entry("b", 2))).stream().collect(Collectors.toMap(Map.Entry::getKey, Map.Entry::getValue, (a, b) -> b, HashMap::new));
        List<String> keys = scores.entrySet().stream().map(pair -> {
            String k = pair.getKey();
            int v = pair.getValue();
            return k + v;
        }).collect(Collectors.toCollection(ArrayList::new));
        keys.sort(null);
        System.out.println(keys);
        for (int i = 0; i < edges.size(); i++) {
            Edge e = edges.get(i);
            if (i == 0) {
                System.out.println(e.from() + " " + e.to());
            }
        }
    }

    record Pair(int first, int second) {}

    record Edge(int from, int to, long w) {}

    record Pos(int r, int c) {}
}
