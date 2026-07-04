import SwiftUI

struct LabeledInput: View {
    let title: String
    @Binding var value: Int

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title)
                .font(.caption)
            TextField("", value: $value, format: .number)
                .textFieldStyle(.roundedBorder)
                .frame(width: 110)
        }
    }
}

struct LabeledDoubleInput: View {
    let title: String
    let placeholder: String
    let width: CGFloat
    @Binding var value: Double

    init(title: String, placeholder: String = "", width: CGFloat = 110, value: Binding<Double>) {
        self.title = title
        self.placeholder = placeholder
        self.width = width
        self._value = value
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title)
                .font(.caption)
            TextField(placeholder, value: $value, format: .number)
                .textFieldStyle(.roundedBorder)
                .frame(width: width)
        }
    }
}

struct LabeledToggle: View {
    let title: String
    let width: CGFloat
    @Binding var isOn: Bool

    init(title: String, width: CGFloat = 120, isOn: Binding<Bool>) {
        self.title = title
        self.width = width
        self._isOn = isOn
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title)
                .font(.caption)
            Toggle("", isOn: $isOn)
                .labelsHidden()
                .toggleStyle(.switch)
                .frame(width: width, alignment: .leading)
        }
    }
}

struct MetricBox: View {
    let title: String
    let value: String
    var tint: Color = .gray

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title)
                .font(.caption)
                .foregroundStyle(tint.opacity(0.6))
            Text(value)
                .font(.title3.monospacedDigit())
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(tint.opacity(0.08))
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }
}
