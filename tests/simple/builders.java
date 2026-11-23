import java.util.List;

class Main {
    static void emit(StringBuilder out, int depth, String word) {
        for (int j = 0; j < depth; j++) {
            out.append("  ");
        }
        out.append(word);
        out.append('\n');
    }

    static void render(List<String> items, StringBuilder out) {
        for (int i = 0; i < items.size(); i++) {
            String w = items.get(i);
            emit(out, i, w);
        }
        if (out.toString().endsWith("\n")) {
            out.setLength(Math.max(0, out.length() - 1));
        }
    }

    public static void main(String[] args) {
        StringBuilder s = new StringBuilder();
        render(List.of("a", "b", "c"), s);
        System.out.println(s);
        System.out.println(s.length());
    }
}
