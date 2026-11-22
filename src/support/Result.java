package PACKAGE;

import java.util.Optional;
import java.util.function.Consumer;
import java.util.function.Function;
import java.util.function.Supplier;

public sealed interface Result<T, E> permits Ok, Err {
    default String debug() {
        return switch (this) {
            case Ok<T, E>(var v) -> "Ok(" + v + ")";
            case Err<T, E>(var e) -> "Err(" + e + ")";
        };
    }
    default boolean is_ok() { return this instanceof Ok; }
    default boolean is_err() { return this instanceof Err; }
    default T unwrap() {
        return switch (this) {
            case Ok<T, E>(var v) -> v;
            case Err<T, E>(var e) -> throw new RuntimeException("called `Result::unwrap()` on an `Err` value: " + e);
        };
    }
    default T unwrap(Function<E, String> debug) {
        return switch (this) {
            case Ok<T, E>(var v) -> v;
            case Err<T, E>(var e) -> throw new RuntimeException("called `Result::unwrap()` on an `Err` value: " + debug.apply(e));
        };
    }
    default T expect(String msg) {
        return switch (this) {
            case Ok<T, E>(var v) -> v;
            case Err<T, E>(var e) -> throw new RuntimeException(msg + ": " + e);
        };
    }
    default T expect(String msg, Function<E, String> debug) {
        return switch (this) {
            case Ok<T, E>(var v) -> v;
            case Err<T, E>(var e) -> throw new RuntimeException(msg + ": " + debug.apply(e));
        };
    }
    default E unwrap_err() {
        return switch (this) {
            case Ok<T, E>(var v) -> throw new RuntimeException("called `Result::unwrap_err()` on an `Ok` value: " + v);
            case Err<T, E>(var e) -> e;
        };
    }
    default E expect_err(String msg, Function<T, String> debug) {
        return switch (this) {
            case Ok<T, E>(var v) -> throw new RuntimeException(msg + ": " + debug.apply(v));
            case Err<T, E>(var e) -> e;
        };
    }
    default Result<T, E> inspect_err(Consumer<E> f) {
        if (this instanceof Err<T, E>(var e)) {
            f.accept(e);
        }
        return this;
    }
    default T unwrap_or(T other) { return this instanceof Ok<T, E>(var v) ? v : other; }
    default T unwrap_or_else(Function<E, T> f) {
        return switch (this) {
            case Ok<T, E>(var v) -> v;
            case Err<T, E>(var e) -> f.apply(e);
        };
    }
    default Optional<T> ok() { return this instanceof Ok<T, E>(var v) ? Optional.of(v) : Optional.empty(); }
    default Optional<E> err() { return this instanceof Err<T, E>(var e) ? Optional.of(e) : Optional.empty(); }
    default <U> Result<U, E> map(Function<T, U> f) {
        return switch (this) {
            case Ok<T, E>(var v) -> new Ok<>(f.apply(v));
            case Err<T, E>(var e) -> new Err<>(e);
        };
    }
    default <F> Result<T, F> map_err(Function<E, F> f) {
        return switch (this) {
            case Ok<T, E>(var v) -> new Ok<>(v);
            case Err<T, E>(var e) -> new Err<>(f.apply(e));
        };
    }
    default <U> Result<U, E> and_then(Function<T, Result<U, E>> f) {
        return switch (this) {
            case Ok<T, E>(var v) -> f.apply(v);
            case Err<T, E>(var e) -> new Err<>(e);
        };
    }
    default <F> Result<T, F> or_else(Function<E, Result<T, F>> f) {
        return switch (this) {
            case Ok<T, E>(var v) -> new Ok<>(v);
            case Err<T, E>(var e) -> f.apply(e);
        };
    }
}

record Ok<T, E>(T value) implements Result<T, E> {
    public String debug() { return "Ok(" + value + ")"; }
}

record Err<T, E>(E error) implements Result<T, E> {
    public String debug() { return "Err(" + error + ")"; }
}
