#include <lib/std.mi>

Function int getBirtday_Day(int day, int month);
Function String getBirtday_String(int day, int month, int year);

Global Group XUIGroup;
Global Text name, age, ageTag, alias, country, credits, birthday;
Global Layer mugshot, anibg;

System.onScriptLoaded(){
	XUIGroup = getScriptGroup();
	name = XUIGroup.findObject("about.name");
	age = XUIGroup.findObject("about.age");
	ageTag = XUIGroup.findObject("about.age.tag");
	alias = XUIGroup.findObject("about.alias");
	country = XUIGroup.findObject("about.country");
	mugshot = XUIGroup.findObject("about.mugshot");
	credits = XUIGroup.findObject("about.credits");
}

System.onSetXuiParam(String param, String value) {
	if(strlower(param) == "about_name"){
		name.setText(value);
	}
	else if(strlower(param) == "about_age"){
		int calc_age = 1899+System.getDateYear(System.getDate())-stringToInteger(getToken(value, ";", 2));
		int dayOfYear = System.getDateDoy(System.getDate());

		//boolean skrikeljaar;
		if(System.getDateYear(System.getDate())%4==0 && dayOfYear > 58) dayOfYear--;
		//if(System.getDateYear(System.getDate())%4==0) skrikeljaar=true;
		
		if(getBirtday_Day(stringToInteger(getToken(value, ";", 0)), stringToInteger(getToken(value, ";", 1)))<=dayOfYear){
			calc_age++;
		}
		
		/*if(stringToInteger(getToken(value, ";", 2))>=2012){
			age.setXmlParam("x", "130");
			name.setXmlParam("x", "130");
			alias.setXmlParam("x", "130");
			country.setXmlParam("x", "130");
			
			ageTag.setText("Released:");
		}*/
		
		if(getToken(value, ";", 2)=="") age.setText(getBirtday_String(stringToInteger(getToken(value, ";", 0)), stringToInteger(getToken(value, ";", 1)), stringToInteger(getToken(value, ";", 2))));
		else age.setText(getBirtday_String(stringToInteger(getToken(value, ";", 0)), stringToInteger(getToken(value, ";", 1)), stringToInteger(getToken(value, ";", 2)))+" ("+integerToString(calc_age)+")");
		//debugInt(System.getDateDoy(System.getDate()));
		//debugInt(calc_age);
		//debugInt(getBirtday_Day(stringToInteger(getToken(value, ";", 0)), stringToInteger(getToken(value, ";", 1))));

		//debug(integerToString(getBirtday_Day(stringToInteger(getToken(value, ";", 0)), stringToInteger(getToken(value, ";", 1))))+" vandag="+integerToString(System.getDateDoy(System.getDate()))+" naam="+name.getText());
		//debugInt(System.getDateDoy(System.getDate()));
		
		if(getBirtday_Day(stringToInteger(getToken(value, ";", 0)), stringToInteger(getToken(value, ";", 1)))==dayOfYear){
			birthday = XUIGroup.getParent().getParent().findObject("hbd.message");
			birthday.setText("Happy Birthday");
			birthday.show();
			birthday = XUIGroup.getParent().getParent().findObject("hbd.message2");
			birthday.setText(name.getText()+"!!!");
			birthday.show();
			
			anibg = XUIGroup.getParent().getParent().findObject("about.gfx4ani");
			anibg.setXmlParam("image", "about.hbd");
		}
	}
	else if(strlower(param) == "about_alias"){
		alias.setText(value);
	}
	else if(strlower(param) == "about_country"){
		country.setText(value);
	}
	else if(strlower(param) == "about_image"){
		mugshot.setXmlParam("image", value);
	}
	else if(strlower(param) == "credits"){
		credits.setXmlParam("text", value);
	}
	
}

getBirtday_Day(int day, int month){
	int output=-1;

	if(month>1) output+=31;
	if(month>2) output+=28;
	if(month>3) output+=31;
	if(month>4) output+=30;
	if(month>5) output+=31;
	if(month>6) output+=30;
	if(month>7) output+=31;
	if(month>8) output+=31;
	if(month>9) output+=30;
	if(month>10) output+=31;
	if(month>11) output+=30;

	output+=day;

	return output;
}

getBirtday_String(int day, int month, int year){
	String output = integerToString(day)+"-";
	if(month==1) output+="Jan";
	else if(month==2) output+="Feb";
	else if(month==3) output+="Mar";
	else if(month==4) output+="Apr";
	else if(month==5) output+="May";
	else if(month==6) output+="Jun";
	else if(month==7) output+="Jul";
	else if(month==8) output+="Aug";
	else if(month==9) output+="Sep";
	else if(month==10) output+="Oct";
	else if(month==11) output+="Nov";
	else if(month==12) output+="Dec";

	if(year!=0)	output+="-"+integerToString(year);

	return output;
}