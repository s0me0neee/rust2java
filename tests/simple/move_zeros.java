import java.util.ArrayList;
import java.util.List;
import java.util.stream.Collectors;

class Main {
    // `Vec::resize` as grow/shrink loops.
    static List<Integer> move_zeros(List<Integer> arr) {
        List<Integer> out = arr.stream().filter(e -> e != 0).collect(Collectors.toCollection(ArrayList::new));
        while (out.size() < arr.size()) {
            out.add(0);
        }
        while (out.size() > arr.size()) {
            out.remove(out.size() - 1);
        }
        return out;
    }

    public static void main(String[] args) {
        System.out.println(move_zeros(List.of(1, 2, 0, 1, 0, 1, 0, 3, 0, 1)));
    }
}
