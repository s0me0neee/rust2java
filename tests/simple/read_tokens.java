import java.io.BufferedInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.util.ArrayList;
import java.util.Collections;
import java.util.stream.Collectors;

class Main {
    static final InputStream STDIN = new BufferedInputStream(System.in);

    // Contest-style token reader: a byte at a time from stdin.
    static String read_token() throws IOException {
        var token = new ArrayList<Integer>();
        var buf = new ArrayList<Integer>(Collections.nCopies(1, 0));

        while (true) {
            byte[] bytes = new byte[buf.size()];
            int n = Math.max(0, STDIN.read(bytes));
            for (int i = 0; i < n; i++) {
                buf.set(i, bytes[i] & 255);
            }
            if (n == 0 || !Character.isWhitespace(buf.get(0))) {
                break;
            }
        }
        token.add(buf.get(0));

        while (true) {
            byte[] bytes = new byte[buf.size()];
            int n = Math.max(0, STDIN.read(bytes));
            for (int i = 0; i < n; i++) {
                buf.set(i, bytes[i] & 255);
            }
            if (n == 0 || Character.isWhitespace(buf.get(0))) {
                break;
            }
            token.add(buf.get(0));
        }

        return token.stream().map(b -> String.valueOf((char) (int) b)).collect(Collectors.joining());
    }

    public static void main(String[] args) throws IOException {
        var n = Integer.parseInt(read_token());
        long total = 0L;
        for (int i = 0; i < n; i++) {
            String s = read_token();
            long digits = (long) s.chars().mapToObj(c2 -> (char) c2).filter(Character::isDigit).count();
            total += digits * Long.parseLong(read_token());
        }
        System.out.println(total);
    }
}
