import java.util.Comparator;
import java.util.HashMap;
import java.util.List;
import java.util.PriorityQueue;

// Records for immutable structs, map lookups and heap pops as nullable locals, Debug as toString.
class Main {
    record Item(String name, int price) {
        public String toString() {
            return "Item { name: " + debug_str(this.name, '"') + ", price: " + this.price + " }";
        }
    }

    enum Size {
        Small, Large
    }

    static Size size_of(int price) {
        if (price >= 0 && price <= 9) {
            return Size.Small;
        } else {
            return Size.Large;
        }
    }

    static String label(Size size) {
        return switch (size) {
            case Small -> "small";
            case Large -> "large";
        };
    }

    public static void main(String[] args) {
        var stock = new HashMap<String, Item>();
        for (Pair item2 : List.of(new Pair("pen", 3), new Pair("lamp", 40))) {
            String name = item2.name();
            int price = item2.price();
            stock.put(name, new Item(name, price));
        }

        for (String key : List.of("pen", "desk")) {
            Item item = stock.get(key);
            if (item != null) {
                System.out.println(item.name + " costs " + item.price + " (" + label(size_of(item.price)) + ")");
            } else {
                System.out.println("no " + key);
            }
        }
        if (stock.containsKey("lamp")) {
            Item item = stock.get("lamp");
            System.out.println(item);
        }

        var heap = new PriorityQueue<Integer>(Comparator.reverseOrder());
        for (int p : List.of(5, 1, 9)) {
            heap.add(p);
        }
        Integer top;
        while ((top = heap.poll()) != null) {
            System.out.println(top + " " + size_of(top));
        }
    }

    static String debug_str(String s, char q) {
        StringBuilder b = new StringBuilder().append(q);
        s.codePoints().forEach(c -> b.append(c == q ? "\\" + q : c == '\\' ? "\\\\" : c == '\n' ? "\\n" : c == '\r' ? "\\r" : c == '\t' ? "\\t" : c == 0 ? "\\0" : Character.isISOControl(c) ? "\\u{" + Integer.toHexString(c) + "}" : Character.toString(c)));
        return b.append(q).toString();
    }

    record Pair(String name, int price) {}
}
