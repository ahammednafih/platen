package com.kanamlabs.platen

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialExpressiveTheme
import androidx.compose.material3.MotionScheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp

enum class Accent(val label: String, val light: Color, val dark: Color, val onLight: Color, val onDark: Color) {
    Ink("Ink black", Color(0xFF1C1D1F), Color(0xFFE8E6E1), Color.White, Color(0xFF111214)),
    Magenta("Magenta", Color(0xFFC2185B), Color(0xFFEC6A96), Color.White, Color(0xFF1A0710)),
    Orange("Toner orange", Color(0xFFBF3F0C), Color(0xFFF2703A), Color.White, Color(0xFF1B120C)),
    Teal("Teal", Color(0xFF0F766E), Color(0xFF2BB3A3), Color.White, Color(0xFF04201D)),
    Coffee("Coffee", Color(0xFF7A4A2A), Color(0xFFB8835D), Color.White, Color(0xFF1B120C)),
    Wallpaper("Match wallpaper", Color(0xFF6C6F75), Color(0xFF8A8F98), Color.White, Color.Black),
}

private fun inter(weight: Int) = Font(R.font.inter, FontWeight(weight), variationSettings = FontVariation.Settings(FontVariation.weight(weight)))

private val Inter = FontFamily(inter(400), inter(500), inter(600), inter(700), inter(800))

private fun style(size: Int, line: Int, weight: Int, tracking: Double = 0.0) =
    TextStyle(fontFamily = Inter, fontSize = size.sp, lineHeight = line.sp, fontWeight = FontWeight(weight), letterSpacing = tracking.em)

private val PlatenType = Typography(
    displayLarge = style(52, 58, 700, -0.03),
    displayMedium = style(42, 48, 700, -0.025),
    displaySmall = style(34, 40, 700, -0.02),
    headlineLarge = style(30, 36, 700, -0.02),
    headlineMedium = style(26, 32, 700, -0.015),
    headlineSmall = style(22, 28, 650, -0.01),
    titleLarge = style(20, 26, 650, -0.01),
    titleMedium = style(16, 22, 600, -0.005),
    titleSmall = style(14, 20, 600),
    bodyLarge = style(16, 24, 400),
    bodyMedium = style(14, 20, 400),
    bodySmall = style(12, 16, 450),
    labelLarge = style(14, 20, 550),
    labelMedium = style(12, 16, 550),
    labelSmall = style(11, 16, 550),
)

private val PlatenShapes = Shapes(
    extraSmall = RoundedCornerShape(6.dp),
    small = RoundedCornerShape(10.dp),
    medium = RoundedCornerShape(16.dp),
    large = RoundedCornerShape(24.dp),
    extraLarge = RoundedCornerShape(32.dp),
)

private fun light(a: Accent): ColorScheme {
    val soft = a.light.copy(alpha = 0.10f).compositeOver(Color.White)
    return lightColorScheme(
        primary = a.light, onPrimary = a.onLight,
        primaryContainer = soft, onPrimaryContainer = a.light,
        secondary = Color(0xFF55585E), onSecondary = Color.White,
        secondaryContainer = a.light.copy(alpha = 0.12f).compositeOver(Color(0xFFF1F1F3)), onSecondaryContainer = a.light,
        tertiary = a.light, onTertiary = a.onLight,
        tertiaryContainer = soft, onTertiaryContainer = a.light,
        background = Color(0xFFF7F7F8), onBackground = Color(0xFF1C1D1F),
        surface = Color(0xFFF7F7F8), onSurface = Color(0xFF1C1D1F),
        surfaceVariant = Color(0xFFECECEF), onSurfaceVariant = Color(0xFF595C62),
        surfaceContainerLowest = Color.White, surfaceContainerLow = Color(0xFFFFFFFF),
        surfaceContainer = Color(0xFFF1F1F3), surfaceContainerHigh = Color(0xFFECECEF), surfaceContainerHighest = Color(0xFFE5E5E9),
        surfaceBright = Color.White, surfaceDim = Color(0xFFE7E7EA),
        outline = Color(0xFF84878E), outlineVariant = Color(0xFFE2E2E6),
        error = Color(0xFFD14343), onError = Color.White, errorContainer = Color(0xFFFBE4E4), onErrorContainer = Color(0xFF8E1F1F),
        inverseSurface = Color(0xFF1C1D1F), inverseOnSurface = Color(0xFFF1F1F3), inversePrimary = a.dark,
    )
}

private fun dark(a: Accent): ColorScheme {
    val soft = a.dark.copy(alpha = 0.16f).compositeOver(Color(0xFF151618))
    return darkColorScheme(
        primary = a.dark, onPrimary = a.onDark,
        primaryContainer = soft, onPrimaryContainer = a.dark,
        secondary = Color(0xFFA9ADB5), onSecondary = Color(0xFF111214),
        secondaryContainer = a.dark.copy(alpha = 0.18f).compositeOver(Color(0xFF1C1D21)), onSecondaryContainer = a.dark,
        tertiary = a.dark, onTertiary = a.onDark,
        tertiaryContainer = soft, onTertiaryContainer = a.dark,
        background = Color(0xFF101113), onBackground = Color(0xFFE3E4E6),
        surface = Color(0xFF101113), onSurface = Color(0xFFE3E4E6),
        surfaceVariant = Color(0xFF1F2024), onSurfaceVariant = Color(0xFFA3A8B0),
        surfaceContainerLowest = Color(0xFF0C0D0E), surfaceContainerLow = Color(0xFF151618),
        surfaceContainer = Color(0xFF0C0D0E), surfaceContainerHigh = Color(0xFF1C1D21), surfaceContainerHighest = Color(0xFF26272C),
        surfaceBright = Color(0xFF26272C), surfaceDim = Color(0xFF0C0D0E),
        outline = Color(0xFF686B73), outlineVariant = Color(0xFF26282D),
        error = Color(0xFFEB5757), onError = Color(0xFF1B0B0B), errorContainer = Color(0xFF3A1717), onErrorContainer = Color(0xFFF5B4B4),
        inverseSurface = Color(0xFFE3E4E6), inverseOnSurface = Color(0xFF151618), inversePrimary = a.light,
    )
}

@Composable
fun PlatenTheme(accent: Accent, content: @Composable () -> Unit) {
    val context = LocalContext.current
    val darkMode = isSystemInDarkTheme()
    val scheme = when {
        accent == Accent.Wallpaper && darkMode -> dynamicDarkColorScheme(context)
        accent == Accent.Wallpaper -> dynamicLightColorScheme(context)
        darkMode -> dark(accent)
        else -> light(accent)
    }
    MaterialExpressiveTheme(colorScheme = scheme, typography = PlatenType, shapes = PlatenShapes, motionScheme = MotionScheme.expressive(), content = content)
}
