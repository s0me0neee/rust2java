package PACKAGE;

import java.util.Optional;
import java.util.function.Function;
import java.util.function.Supplier;

public sealed interface Result<T, E> permits Ok, Err {
    default String debug() {
        return this instanceof Ok<?, ?>(var v) ? "Ok(" + v + ")" : "Err(" + ((Err<?, ?>) this).error() + ")";
    }
    default boolean is_ok() { return this instanceof Ok; }
    default boolean is_err() { return this instanceof Err; }
    default T unwrap() {
        return switch (this) {
            case Ok<T, E>(var v) -> v;
            case Err<T, E>(var e) -> throw new RuntimeException("called `Result::unwrap()` on an `Err` value: " + e);
        };
    }
    default T expect(String msg) {
        return switch (this) {
            case Ok<T, E>(var v) -> v;
            case Err<T, E>(var e) -> throw new RuntimeException(msg + ": " + e);
        };
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
}

record Ok<T, E>(T value) implements Result<T, E> {
    public String debug() { return "Ok(" + value + ")"; }
}

record Err<T, E>(E error) implements Result<T, E> {
    public String debug() { return "Err(" + error + ")"; }
}
