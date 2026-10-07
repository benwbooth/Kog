package org.kog.player

import org.json.JSONArray
import org.json.JSONObject
import kotlin.math.roundToInt

private fun JSONArray?.objects(): List<JSONObject> = if (this == null) emptyList() else
    (0 until length()).mapNotNull { optJSONObject(it) }
internal data class InspectionField(val name: String, val value: String)
internal data class InspectionNote(val key: Float, val held: Boolean, val velocity: Float)
internal data class InspectionChannel(val id: Int, val name: String, val kind: String, val instrument: String,
    val active: Boolean, val level: Float, val notes: List<InspectionNote>, val fields: List<InspectionField>) {
    companion object { fun parse(o: JSONObject) = InspectionChannel(o.optInt("id"), o.optString("name"),
        o.optString("kind"), o.optString("instrument"), o.optBoolean("active"), o.optDouble("level",0.0).toFloat(),
        o.optJSONArray("notes").objects().map { InspectionNote(it.optDouble("key").toFloat(),it.optBoolean("held"),it.optDouble("velocity",1.0).toFloat()) }, fields(o.optJSONArray("fields"))) }
}
internal fun fields(a: JSONArray?) = a.objects().map { InspectionField(it.optString("name"),it.optString("value")) }
internal fun InspectionField.label() = "$name: $value"
internal data class InspectionCell(val channel: Int, val notes: String, val instrument: String, val volume: String, val effects: List<InspectionField>)
internal data class InspectionRow(val time: Double, val label: String, val cells: List<InspectionCell>, val global: List<InspectionField>) {
    companion object { fun parse(o: JSONObject) = InspectionRow(o.optDouble("time"),o.optString("label"),
        o.optJSONArray("cells").objects().map { InspectionCell(it.optInt("channel"),it.optString("notes"),it.optString("instrument"),it.optString("volume"),fields(it.optJSONArray("effects"))) },fields(o.optJSONArray("global"))) }
}
internal data class InspectionSnapshot(val backend: String = "", val detail: String = "Play a track to inspect its channels.",
    val channels: List<InspectionChannel> = emptyList(), val rows: List<InspectionRow> = emptyList(), val current: Int? = null,
    val global: List<InspectionField> = emptyList()) {
    companion object { fun parse(o: JSONObject) = InspectionSnapshot(o.optJSONObject("description")?.optString("backend").orEmpty(),
        o.optJSONObject("description")?.optString("detail").orEmpty(), o.optJSONArray("channels").objects().map(InspectionChannel::parse),
        o.optJSONArray("rows").objects().map(InspectionRow::parse),if (o.isNull("current_row")) null else o.optInt("current_row"),fields(o.optJSONArray("global"))) }
}
internal class InspectionWindow(private val json: JSONObject) {
    val start = json.optDouble("start")
    val end = json.optDouble("end")
    fun snapshot(position: Double): InspectionSnapshot {
        if (position < start || position >= end) return InspectionSnapshot()
        val initial = json.optJSONObject("initial") ?: JSONObject()
        val channels = initial.optJSONArray("channels").objects().map(InspectionChannel::parse).associateBy { it.id }.toMutableMap()
        var global = fields(initial.optJSONArray("global"))
        val rows = json.optJSONArray("rows").objects().map(InspectionRow::parse).toMutableList()
        for (frame in json.optJSONArray("frames").objects()) {
            if (frame.optDouble("time") <= position + 0.000001) {
                for (channel in frame.optJSONArray("channels").objects().map(InspectionChannel::parse)) channels[channel.id] = channel
                frame.optJSONArray("removed")?.let { removed -> for (i in 0 until removed.length()) channels.remove(removed.optInt(i)) }
                frame.optJSONArray("global")?.let { global = fields(it) }
            }
            frame.optJSONObject("row")?.let { row ->
                val parsed = InspectionRow.parse(row)
                if (rows.lastOrNull()?.let { it.label == parsed.label && it.cells == parsed.cells && it.global == parsed.global } != true) rows.add(parsed)
            }
        }
        val cursor = rows.indexOfLast { it.time <= position + 0.000001 }
        val begin = (cursor + 1 - 24).coerceAtLeast(0)
        return InspectionSnapshot(json.optJSONObject("description")?.optString("backend").orEmpty(),
            json.optJSONObject("description")?.optString("detail").orEmpty(),channels.toSortedMap().values.toList(),rows.drop(begin).take(48),
            (cursor - begin).takeIf { it >= 0 }, global)
    }
}
internal fun noteName(key: Float): String {
    if (!key.isFinite()) return "—"
    val keyNumber = key.roundToInt()
    return listOf("C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B")[Math.floorMod(keyNumber,12)] + (Math.floorDiv(keyNumber,12)-1)
}
