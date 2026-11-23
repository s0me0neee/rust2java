import java.util.Comparator;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.PriorityQueue;

class Main {
    // Records for immutable structs, map lookups and heap pops as nullable locals, Debug as toString.

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
        for (Map.Entry<String, Integer> item2 : List.of(Map.entry("pen", 3), Map.entry("lamp", 40))) {
            String name = item2.getKey();
            int price = item2.getValue();
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
}
