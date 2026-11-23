import java.util.ArrayList;
import java.util.List;

class Main {
    // A user error enum carried by an exception class; `?`, `match` and `unwrap_or` on its Results.

    sealed interface AppError {
        default String display() {
            var f = new StringBuilder();
            switch (this) {
                case AppError.Negative(var n) -> f.append("negative: " + n);
                case AppError.Empty() -> f.append("empty");
            }
            return f.toString();
        }

        record Negative(int _0) implements AppError {
            public String debug() {
                return "Negative(" + this._0 + ")";
            }

            public String toString() {
                return display();
            }
        }

        record Empty() implements AppError {
            public String debug() {
                return "Empty";
            }

            public String toString() {
                return display();
            }
        }

        String debug();
    }

    static int parse(String s) {
        if (s.isEmpty()) {
            throw new AppErrorException(new AppError.Empty());
        }
        var n = Integer.parseInt(s.strip());
        if (n < 0) {
            throw new AppErrorException(new AppError.Negative(n));
        }
        return n;
    }

    static int total(List<String> xs) {
        int t = 0;
        for (String x : xs) {
            t += parse(x);
        }
        return t;
    }

    public static void main(String[] args) {
        System.out.println(total(new ArrayList<>(List.of("1", "2"))));

        try {
            int n = parse("-3");
            System.out.println("ok " + n);
        } catch (AppErrorException ex) {
            AppError e = ex.error;
            System.out.println("error: " + e);
        }
        int v;
        try {
            v = parse("");
        } catch (AppErrorException e2) {
            v = 0;
        }
        try {
            int n = parse("5");
            System.out.println("five " + n);
        } catch (AppErrorException e2) {}
        System.out.println(v);
    }

    static class AppErrorException extends RuntimeException {
        final AppError error;

        AppErrorException(AppError error) {
            super(String.valueOf(error));
            this.error = error;
        }
    }
}
