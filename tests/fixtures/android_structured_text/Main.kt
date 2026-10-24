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
        verifyBooleanScalars()
    } else {
        val failure = assertFailsWith<java.lang.reflect.InvocationTargetException> { method.invoke(suite) }
        check(failure.cause is AssertionError) { "wrong failure for $case: ${failure.cause}" }
    }
    println("android_full_dispatch_case=" + case.ifEmpty { "valid" })
}

private fun verifyBooleanScalars() {
    val nativeValue = SampleCore.matrixTruth().content
    check(nativeValue is BodyValue.Boolean && nativeValue.value)
    val mapper = com.fasterxml.jackson.databind.ObjectMapper().findAndRegisterModules()
    for ((json, expected) in listOf("true" to true, "false" to false)) {
        val decoded = mapper.readValue(json, BodyValue::class.java)
        check(decoded is BodyValue.Boolean && decoded.value == expected)
        check(mapper.writeValueAsString(decoded) == json)
    }
    val numeric = mapper.readValue("1", BodyValue::class.java)
    check(numeric is BodyValue.Count && numeric.value == 1L)
    val text = mapper.readValue("\"true\"", BodyValue::class.java)
    check(text is BodyValue.Text && text.value == "true")
    println("android_boolean_roundtrips=2 jni_boolean_cases=1 non_boolean_controls=2")
}
