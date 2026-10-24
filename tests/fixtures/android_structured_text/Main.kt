package sample

import kotlin.test.assertFailsWith
import org.junit.jupiter.api.Test
import sample.e2e.StructuredTest

fun main() {
    val suite = StructuredTest()
    val method = StructuredTest::class.java.declaredMethods.single { it.isAnnotationPresent(Test::class.java) }
    val case = System.getenv("ALEF_STRUCTURED_TEXT_CASE").orEmpty()
    if (case.isEmpty()) {
        method.invoke(suite)
    } else {
        val failure = assertFailsWith<java.lang.reflect.InvocationTargetException> { method.invoke(suite) }
        check(failure.cause is AssertionError) { "wrong failure for $case: ${failure.cause}" }
    }
    println("android_full_dispatch_case=" + case.ifEmpty { "valid" })
}
